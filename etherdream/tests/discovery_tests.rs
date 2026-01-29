use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4 };
use std::sync::{ Arc, RwLock };

use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time;

use etherdream::{ constants::*, device, discovery };

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Test Helpers

struct TestDiscoveryServer {
  server: Arc<RwLock<Option<discovery::Server>>>,
  _timeout_handle: JoinHandle<()>
}

impl TestDiscoveryServer {
  // Starts a test discovery server. The test discovery is programmed to
  // terminate after a limited number of secs. This function will return a
  // tuple containing:
  // (1) the test discovery server, and
  // (2) an mpsc receiver that the caller can use to receive discovered devices
  async fn start() -> ( Self, mpsc::Receiver<device::Device> ) {
    let ( discovered_device_tx, discovered_device_rx ) = mpsc::channel::<device::Device>( 16 );

    // Create a discovery server on any available port. Will notify the creator
    // of any discovered devices via `discovered_device_rx`.
    let server_fut = {
      discovery::Server::serve_with_address(
        SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 ), discovered_device_tx
      )
    };

    if let  Ok( server ) = server_fut.await {
      let server_ref = Arc::new( RwLock::new( Some( server ) ) );

      // Establish a timeout task that will shut the discovery down after a
      // small duration of time.
      let timeout_handle = tokio::spawn({
        let server_ref = server_ref.clone();

        async move {
          time::sleep( time::Duration::from_secs( 5 ) ).await;

          let server = server_ref.write().unwrap().take();
          if let Some( server ) = server {
            let _ = server.shutdown().await;
          }
        }
      });

      ( Self{ _timeout_handle: timeout_handle, server: server_ref }, discovered_device_rx )
    } else {
      panic!( "Failed to create the test discovery server." )
    }
  }

  fn address( &self ) -> SocketAddr {
    match self.server.read().unwrap().as_ref() {
      Some( server ) => server.address(),
      None => panic!( "The discovery server has already been shutdown." )
    }
  }
}

// Broadcasts `count` number of test device messages to a discovery `server`.
async fn broadcast_device( server: &TestDiscoveryServer, intrinsics: &device::Intrinsics, state: &device::State, count: usize ) -> io::Result<()> {
  let local_socket = UdpSocket::bind( SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 ) ).await?;
  local_socket.set_broadcast( true )?;

  let mut buf = [0u8; ETHERDREAM_BROADCAST_BYTES];
  copy_into_etherdream_broadcast_bytes( &mut buf, &intrinsics, &state );

  for i in 0..count {
    if let Err( err ) = local_socket.send_to( &buf, server.address() ).await {
      panic!( "Failed to broadcast device message {i} of {count}: {err}" );
    }
  }

  Ok(())
}

// Awaits for a single device or panics. A panic can happen if a test timeout
// is reached or the server was shutdown from some other means.
async fn receive_device_or_panic( rx: &mut mpsc::Receiver<device::Device> ) -> device::Device {
  if let Some( discovered_device ) = rx.recv().await {
    discovered_device
  } else {
    panic!( "Failed to receive device..." )
  }
}

fn copy_into_etherdream_broadcast_bytes( buf: &mut [u8], intrinsics: &device::Intrinsics, state: &device::State ) {
  // Copy intrinsics into `buf`
  buf[0..6].copy_from_slice( intrinsics.mac_address.as_slice() );
  buf[6..8].copy_from_slice( &intrinsics.version.hardware.to_le_bytes() );
  buf[8..10].copy_from_slice( &intrinsics.version.software.to_le_bytes() );
  buf[10..12].copy_from_slice( &intrinsics.buffer_capacity.to_le_bytes() );
  buf[12..16].copy_from_slice( &intrinsics.max_points_per_second.to_le_bytes() );

  // Copy state into `buf`
  buf[16] = 0;
  buf[17] = match state.light_engine_state {
    device::LightEngineState::Ready => 0,
    device::LightEngineState::WarmUp => 1,
    device::LightEngineState::CoolDown => 2,
    device::LightEngineState::Estop => 3
  };

  buf[18] = match state.playback_state {
    device::PlaybackState::Idle => 0,
    device::PlaybackState::Prepared => 1,
    device::PlaybackState::Playing => 2
  };

  buf[19] = match state.source {
    device::Source::Network => 0,
    device::Source::Ilda => 1,
    device::Source::Internal => 2,
  };

  buf[20..26].fill( 0 );
  buf[26..28].copy_from_slice( &state.points_buffered.to_le_bytes() );
  buf[28..32].copy_from_slice( &state.points_per_second.to_le_bytes() );
  buf[32..36].copy_from_slice( &state.points_lifetime.to_le_bytes() );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Unit Tests

#[tokio::test]
async fn discovery_server_will_receive_a_single_etherdream_broadcast() -> Result<(),io::Error> {
  let ( server, mut device_rx ) = TestDiscoveryServer::start().await;

  // Create a test device
  let intrinsics = device::Intrinsics{
    buffer_capacity: 16,
    mac_address: [ 0, 1, 2, 3, 4, 5 ].into(),
    max_points_per_second: 128,
    version: [ 3, 2 ].into()
  };

  let state = device::State{
    light_engine_state: device::LightEngineState::Ready,
    playback_state: device::PlaybackState::Prepared,
    points_lifetime: 1234,
    points_per_second: 1024,
    ..Default::default()
  };

  // Broadcast the test device to the discovery server
  broadcast_device( &server, &intrinsics, &state, 1 ).await?;

  // Verify that the discovery server receives that device and executes the callback
  let discovered_device = receive_device_or_panic( &mut device_rx ).await;

  // Verify discovered device attributes
  assert_eq!( discovered_device.address(), SocketAddrV4::new( Ipv4Addr::LOCALHOST, 7765 ).into() );
  assert_eq!( discovered_device.buffer_capacity(), intrinsics.buffer_capacity );
  assert_eq!( discovered_device.mac_address(), intrinsics.mac_address );
  assert_eq!( discovered_device.max_points_per_second(), intrinsics.max_points_per_second );
  assert_eq!( discovered_device.version(), intrinsics.version );

  assert_eq!( discovered_device.state().light_engine_state, device::LightEngineState::Ready );
  assert_eq!( discovered_device.state().playback_state, device::PlaybackState::Prepared );
  assert_eq!( discovered_device.state().points_lifetime, 1234 );
  assert_eq!( discovered_device.state().points_per_second, 1024 );
  assert_eq!( discovered_device.state().source, device::Source::Network );

  Ok(())
}

#[tokio::test]
async fn discovery_server_will_only_execute_callback_once_for_each_unique_device() -> Result<(),io::Error> {
  let ( server, mut device_rx ) = TestDiscoveryServer::start().await;

  let state = device::State::default();

  // Broadcasting this device ten (10) times
  let intrinsics_1 = device::Intrinsics{
    mac_address: [10; 6].into(),
    ..Default::default()
  };

  broadcast_device( &server, &intrinsics_1, &state, 10 ).await?;

  // Broadcasting this device once (1)
  let intrinsics_2 = device::Intrinsics{
    mac_address: [20; 6].into(),
    ..Default::default()
  };

  broadcast_device( &server, &intrinsics_2, &state, 1 ).await?;

  let discovered_device = device_rx.recv().await.unwrap();
  assert_eq!( discovered_device.mac_address(), intrinsics_1.mac_address );

  let discovered_device = device_rx.recv().await.unwrap();
  assert_eq!( discovered_device.mac_address(), intrinsics_2.mac_address );

  Ok(())
}