use tokio::time::{ sleep, Duration };

use etherdream_simulator::{ self as simulator, Simulator };

use etherdream::client::Client;
use etherdream::DeviceInfo;
use etherdream::protocol;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Tests

#[tokio::test]
async fn will_return_device_properties() {
  let ( simulator, client ) = setup().await;

  assert_eq!( client.mac_address(), &simulator.intrinsics().mac_address );
  assert_eq!( client.max_points_per_second(), simulator.intrinsics().max_points_per_second as usize );
  assert_eq!( client.peer_addr(), simulator.address() );

  let state = client.state().await;
  assert_eq!( state.is_ready(), true );
  assert_eq!( state.points_buffered(), 0 );
}

#[tokio::test]
async fn a_ping_will_be_acked() {
  let ( _, mut client ) = setup().await;
  assert!( client.ping().await.is_ok() );
}

#[tokio::test]
async fn a_reset_will_be_acked() {
  let ( _, mut client ) = setup().await;
  assert!( client.reset().await.is_ok() );
}

#[tokio::test]
async fn can_push_and_flush_points() {
  let ( _, mut client ) = setup().await;

  client.push_point( 0, 0, 0, 0, 0 );
  client.push_point( 0, 0, 0, 0, 0 );
  client.push_point( 0, 0, 0, 0, 0 );
  assert_eq!( client.point_count(), 3 );

  let state = client.flush_points().await.unwrap();
  assert_eq!( state.points_buffered(), 3 );
  assert_eq!( client.point_count(), 0 );
}

#[tokio::test]
async fn can_push_and_flush_points_within_the_limits_of_the_devices_capacity() {
  let ( _, mut client ) = setup_with_capacity( 2 ).await;

  // Push four points (two more than the device's capacity)
  client.push_point( 0, 0, 0, 0, 0 );
  client.push_point( 0, 0, 0, 0, 0 );
  client.push_point( 0, 0, 0, 0, 0 );
  client.push_point( 0, 0, 0, 0, 0 );
  assert_eq!( client.point_count(), 4 );

  let state = client.flush_points().await.unwrap();

  // Only two points will be flushed due to the device capacity
  assert_eq!( state.points_buffered(), 2 );
  assert_eq!( client.point_count(), 2 );
}

#[tokio::test]
async fn can_start_the_client() {
  let ( mut simulator, mut client ) = setup_with_capacity( 2 ).await;

  // Push four points (two more than the device's capacity)
  client.push_point( 0, 0, 0, 0, 0 );
  client.push_point( 0, 0, 0, 0, 0 );
  client.push_point( 0, 0, 0, 0, 0 );
  client.push_point( 0, 0, 0, 0, 0 );
  assert_eq!( client.point_count(), 4 );

  let state = client.start( 1000 ).await.unwrap();

  // Only two points will be flushed immediately, due to device capacity
  assert_eq!( state.points_buffered(), 2 );
  assert_eq!( client.point_count(), 2 );

  // We consume all the points from the emulator and wait. During this time:
  // (1) The client writer will auto-ping
  // (2) The client writer will realize point capacity is available
  // (3) The client writer will flush the remaining points
  simulator.consume_points( 2 );
  sleep( Duration::from_secs( 1 ) ).await;

  // Verify that all remaining points have been flushed
  let state = client.state().await;
  assert_eq!( state.points_buffered(), 2 );
  assert_eq!( client.point_count(), 0 );
}

#[tokio::test]
async fn can_stop_the_client() {
  let ( _, mut client ) = setup().await;

  let state = client.start( 1000 ).await.unwrap();
  assert_eq!( state.playback_state(), protocol::PlaybackState::Playing );

  let state = client.stop().await.unwrap();
  assert_eq!( state.playback_state(), protocol::PlaybackState::Idle );
}

#[tokio::test]
async fn can_disconnect_the_client() {
  // TODO
  //let ( _, client ) = setup().await;
  //assert_eq!( client.disconnect().await, () );
}

// Creates a default Etherdream emulator and a client.
async fn setup() -> ( Simulator, Client ) {
  setup_with_capacity( 16 ).await
}

// Creates an Etherdream simulator and client using a provided a point buffer
// `capacity`.
async fn setup_with_capacity( capacity: u16 ) -> ( Simulator, Client  ) {
  let simulator = simulator::Builder::new().capacity( capacity ).start().await.unwrap();
  let device_info = DeviceInfo::new( *simulator.address(), *simulator.intrinsics() );

  let client =
    etherdream::client::Builder::new( device_info )
      .connect().await
      .expect( "Failed to create a client from Etherdream connection" );

  ( simulator, client )
}