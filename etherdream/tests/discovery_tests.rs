use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };

use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time;

use etherdream::discovery;
use etherdream::protocol;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Unit Tests

#[tokio::test]
async fn discovery_server_will_receive_a_single_etherdream_broadcast() -> Result<(),io::Error> {
  let ( discovery_tx, mut discovery_rx ) = mpsc::channel::<discovery::Broadcast>( 16 );

  let discovery = discovery::Discovery::new()
    .address( SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 ) )
    .notify( discovery_tx )
    .listen().await?;

  // Create a test device
  let intrinsics = protocol::Intrinsics{
    buffer_capacity: 16,
    mac_address: [ 0, 1, 2, 3, 4, 5 ].into(),
    max_points_per_second: 128,
    version: [ 3, 2 ].into()
  };

  let state = protocol::State{
    light_engine_state: protocol::LightEngineState::Ready,
    playback_state: protocol::PlaybackState::Prepared,
    points_lifetime: 1234,
    points_per_second: 1024,
    ..Default::default()
  };

  // Broadcast the test device to the discovery server
  let device_addr = broadcast_device( &discovery, &intrinsics, &state, 1 ).await?;
  let broadcast = receive_device_or_panic( &mut discovery_rx ).await;
  assert_eq!( device_addr.ip(), broadcast.device_info().ip() );

  {
    let registry = discovery.registry().read().await;
    let broadcast = registry.get( &device_addr ).expect( "Device not found." );
    assert_eq!( device_addr.ip(), broadcast.device_info().ip() );

    let device_info = broadcast.device_info();
    assert_eq!( device_info.buffer_capacity(), intrinsics.buffer_capacity as usize );
    assert_eq!( *device_info.mac_address(), intrinsics.mac_address );
    assert_eq!( device_info.max_points_per_second(), intrinsics.max_points_per_second as usize );
    assert_eq!( *device_info.version(), intrinsics.version );

    let state = broadcast.state();
    assert_eq!( state.light_engine_state, protocol::LightEngineState::Ready );
    assert_eq!( state.playback_state, protocol::PlaybackState::Prepared );
    assert_eq!( state.points_lifetime, 1234 );
    assert_eq!( state.points_per_second, 1024 );
    assert_eq!( state.source, protocol::Source::Network );
  }

  Ok(())
}

#[tokio::test]
async fn discovery_server_will_only_execute_callback_once_for_each_unique_device() -> Result<(),io::Error> {
  let ( discovery_tx, mut discovery_rx ) = mpsc::channel::<discovery::Broadcast>( 16 );

  let discovery = discovery::Discovery::new()
    .address( SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 ) )
    .notify( discovery_tx )
    .listen().await?;

  let state = protocol::State::default();

  // Broadcasting this device ten (10) times
  let intrinsics_1 = protocol::Intrinsics{
    mac_address: [10; 6].into(),
    ..Default::default()
  };

  let device_addr_1 = broadcast_device( &discovery, &intrinsics_1, &state, 10 ).await?;

  // Broadcasting this device once (1)
  let intrinsics_2 = protocol::Intrinsics{
    mac_address: [20; 6].into(),
    ..Default::default()
  };

  let device_addr_2 = broadcast_device( &discovery, &intrinsics_2, &state, 1 ).await?;

  {
    let broadcast = receive_device_or_panic( &mut discovery_rx ).await;
    assert_eq!( device_addr_1.ip(), broadcast.device_info().ip() );

    let registry = discovery.registry().read().await;
    let broadcast = registry.get( &device_addr_1 ).expect( "Device not found." );
    assert_eq!( *broadcast.device_info().mac_address(), intrinsics_1.mac_address );
  }

  {
    let broadcast = receive_device_or_panic( &mut discovery_rx ).await;
    assert_eq!( device_addr_2.ip(), broadcast.device_info().ip() );

    let registry = discovery.registry().read().await;
    let broadcast = registry.get( &device_addr_2 ).expect( "Device not found." );
    assert_eq!( *broadcast.device_info().mac_address(), intrinsics_2.mac_address );
  }
  Ok(())
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Test Helpers

// Broadcasts `count` number of test device messages to a discovery `server`.
async fn broadcast_device( service: &discovery::Service, intrinsics: &protocol::Intrinsics, state: &protocol::State, count: usize )
  -> io::Result<SocketAddr>
{
  let local_socket = UdpSocket::bind( SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 ) ).await?;
  local_socket.set_broadcast( true )?;

  let mut buf = [0u8; protocol::BROADCAST_BYTES_SIZE];
  copy_into_etherdream_broadcast_bytes( &mut buf, &intrinsics, &state );

  for i in 0..count {
    if let Err( err ) = local_socket.send_to( &buf, service.address() ).await {
      panic!( "Failed to broadcast device message {i} of {count}: {err}" );
    }
  }

  Ok( local_socket.local_addr()? )
}

// Awaits for a single device or panics. A panic can happen if a test timeout
// is reached or the server was shutdown from some other means.
async fn receive_device_or_panic( discovery_rx: &mut mpsc::Receiver<discovery::Broadcast> ) -> discovery::Broadcast {
  tokio::select!{
    _ = time::sleep( time::Duration::from_secs( 5 ) ) => {
      panic!( "Failed to receive a device info by timeout..." )
    }
    broadcast = discovery_rx.recv()  => {
      broadcast.expect( "Discovery service shutdown..." )
    }
  }
}

fn copy_into_etherdream_broadcast_bytes( buf: &mut [u8], intrinsics: &protocol::Intrinsics, state: &protocol::State ) {
  // Copy intrinsics into `buf`
  buf[0..6].copy_from_slice( intrinsics.mac_address.as_slice() );
  buf[6..8].copy_from_slice( &intrinsics.version.hardware.to_le_bytes() );
  buf[8..10].copy_from_slice( &intrinsics.version.software.to_le_bytes() );
  buf[10..12].copy_from_slice( &intrinsics.buffer_capacity.to_le_bytes() );
  buf[12..16].copy_from_slice( &intrinsics.max_points_per_second.to_le_bytes() );

  // Copy state into `buf`
  buf[16] = 0;
  buf[17] = match state.light_engine_state {
    protocol::LightEngineState::Ready => 0,
    protocol::LightEngineState::WarmUp => 1,
    protocol::LightEngineState::CoolDown => 2,
    protocol::LightEngineState::Estop => 3
  };

  buf[18] = match state.playback_state {
    protocol::PlaybackState::Idle => 0,
    protocol::PlaybackState::Prepared => 1,
    protocol::PlaybackState::Playing => 2
  };

  buf[19] = match state.source {
    protocol::Source::Network => 0,
    protocol::Source::Ilda => 1,
    protocol::Source::Internal => 2,
  };

  buf[20..26].fill( 0 );
  buf[26..28].copy_from_slice( &state.points_buffered.to_le_bytes() );
  buf[28..32].copy_from_slice( &state.points_per_second.to_le_bytes() );
  buf[32..36].copy_from_slice( &state.points_lifetime.to_le_bytes() );
}