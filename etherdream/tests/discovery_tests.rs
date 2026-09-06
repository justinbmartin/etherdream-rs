use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };

use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time;

use etherdream::{
  device_info::DeviceInfo,
  discovery::{ self, Discovery },
  protocol::{ BROADCAST_BYTES_SIZE, Intrinsics, LightEngineState, PlaybackState, State, Source } };

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Unit Tests

#[tokio::test]
async fn discovery_server_will_receive_a_single_etherdream_broadcast() -> Result<(),io::Error> {
  let ( device_info_tx, mut device_info_rx ) = mpsc::channel::<DeviceInfo>( 16 );
  let discovery = discovery::Builder::new()
    .address( SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 ) )
    .notify( device_info_tx )
    .listen().await?;

  // Create a test device
  let intrinsics = Intrinsics{
    buffer_capacity: 16,
    mac_address: [ 0, 1, 2, 3, 4, 5 ].into(),
    max_points_per_second: 128,
    version: [ 3, 2 ].into()
  };

  let state = State{
    light_engine_state: LightEngineState::Ready,
    playback_state: PlaybackState::Prepared,
    points_lifetime: 1234,
    points_per_second: 1024,
    ..Default::default()
  };

  // Broadcast the test device to the discovery server
  broadcast_device( &discovery, &intrinsics, &state, 1 ).await?;

  // Verify that the discovery server receives that device and executes the callback
  let device_info = receive_device_or_panic( &mut device_info_rx ).await;

  // Verify discovered device attributes
  assert_eq!( device_info.buffer_capacity(), intrinsics.buffer_capacity as usize );
  assert_eq!( *device_info.mac_address(), intrinsics.mac_address );
  assert_eq!( device_info.max_points_per_second(), intrinsics.max_points_per_second as usize );
  assert_eq!( *device_info.version(), intrinsics.version );

  /// TODO
  //assert_eq!( state.light_engine_state, LightEngineState::Ready );
  //assert_eq!( state.playback_state, PlaybackState::Prepared );
  //assert_eq!( state.points_lifetime, 1234 );
  //assert_eq!( state.points_per_second, 1024 );
  //assert_eq!( state.source, Source::Network );

  Ok(())
}

#[tokio::test]
async fn discovery_server_will_only_execute_callback_once_for_each_unique_device() -> Result<(),io::Error> {
  let ( device_info_tx, mut device_info_rx ) = mpsc::channel::<DeviceInfo>( 16 );
  let discovery = discovery::Builder::new()
    .address( SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 ) )
    .notify( device_info_tx )
    .listen().await?;

  let state = State::default();

  // Broadcasting this device ten (10) times
  let intrinsics_1 = Intrinsics{
    mac_address: [10; 6].into(),
    ..Default::default()
  };

  broadcast_device( &discovery, &intrinsics_1, &state, 10 ).await?;

  // Broadcasting this device once (1)
  let intrinsics_2 = Intrinsics{
    mac_address: [20; 6].into(),
    ..Default::default()
  };

  broadcast_device( &discovery, &intrinsics_2, &state, 1 ).await?;

  let device_info = receive_device_or_panic( &mut device_info_rx ).await;
  assert_eq!( *device_info.mac_address(), intrinsics_1.mac_address );

  let device_info = receive_device_or_panic( &mut device_info_rx ).await;
  assert_eq!( *device_info.mac_address(), intrinsics_2.mac_address );

  Ok(())
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Test Helpers

// Broadcasts `count` number of test device messages to a discovery `server`.
async fn broadcast_device( server: &Discovery, intrinsics: &Intrinsics, state: &State, count: usize ) -> io::Result<()> {
  let local_socket = UdpSocket::bind( SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 ) ).await?;
  local_socket.set_broadcast( true )?;

  let mut buf = [0u8; BROADCAST_BYTES_SIZE];
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
async fn receive_device_or_panic( device_info_rx: &mut mpsc::Receiver<DeviceInfo> ) -> DeviceInfo {
  tokio::select!{
    _ = time::sleep( time::Duration::from_secs( 5 ) ) => {
      panic!( "Failed to receive a device info by timeout..." )
    }
    device_info = device_info_rx.recv()  => {
      device_info.expect( "Discovery service shutdown..." )
    }
  }
}

fn copy_into_etherdream_broadcast_bytes( buf: &mut [u8], intrinsics: &Intrinsics, state: &State ) {
  // Copy intrinsics into `buf`
  buf[0..6].copy_from_slice( intrinsics.mac_address.as_slice() );
  buf[6..8].copy_from_slice( &intrinsics.version.hardware.to_le_bytes() );
  buf[8..10].copy_from_slice( &intrinsics.version.software.to_le_bytes() );
  buf[10..12].copy_from_slice( &intrinsics.buffer_capacity.to_le_bytes() );
  buf[12..16].copy_from_slice( &intrinsics.max_points_per_second.to_le_bytes() );

  // Copy state into `buf`
  buf[16] = 0;
  buf[17] = match state.light_engine_state {
    LightEngineState::Ready => 0,
    LightEngineState::WarmUp => 1,
    LightEngineState::CoolDown => 2,
    LightEngineState::Estop => 3
  };

  buf[18] = match state.playback_state {
    PlaybackState::Idle => 0,
    PlaybackState::Prepared => 1,
    PlaybackState::Playing => 2
  };

  buf[19] = match state.source {
    Source::Network => 0,
    Source::Ilda => 1,
    Source::Internal => 2,
  };

  buf[20..26].fill( 0 );
  buf[26..28].copy_from_slice( &state.points_buffered.to_le_bytes() );
  buf[28..32].copy_from_slice( &state.points_per_second.to_le_bytes() );
  buf[32..36].copy_from_slice( &state.points_lifetime.to_le_bytes() );
}