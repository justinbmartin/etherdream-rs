use tokio::sync::watch;
use tokio::time::{ sleep, Duration };

use etherdream;

pub mod common;
use common::emulator;

#[derive( Clone, Copy, Default )]
struct Point {
  x: i16,
  y: i16,
  r: u16,
  g: u16,
  b: u16
}

impl Point {
  fn new( x: i16, y: i16, r: u16, g: u16, b: u16 ) -> Self {
    Self{ x, y, r, g, b }
  }
}

impl etherdream::Point for Point {
  fn for_etherdream( &self ) -> ( i16, i16, u16, u16, u16 ) {
    ( self.x, self.y, self.r, self.g, self.b )
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Tests

#[tokio::test]
async fn will_return_device_properties() {
  let ( emulator, client ) = make_emulator_and_client().await;
  let mut state = etherdream::device::State::default();

  assert_eq!( client.mac_address(), emulator.device().mac_address() );
  assert_eq!( client.max_points_per_second(), emulator.device().max_points_per_second() as usize );
  assert_eq!( client.peer_addr(), emulator.device().address() );

  client.copy_state( &mut state ).await;
  assert_eq!( state.is_ready(), true );
  assert_eq!( state.points_buffered, 0 );
}

#[tokio::test]
async fn a_ping_will_be_acked() {
  let ( _, mut client ) = make_emulator_and_client().await;
  assert!( client.ping().await.is_ok() );
}

#[tokio::test]
async fn a_reset_will_be_acked() {
  let ( _, mut client ) = make_emulator_and_client().await;
  assert!( client.reset().await.is_ok() );
}

#[tokio::test]
async fn can_push_and_flush_points() {
  let ( _, mut client ) = make_emulator_and_client().await;

  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  assert_eq!( client.point_count(), 3 );

  let state = client.flush_points_and_wait().await.unwrap();
  assert_eq!( state.points_buffered, 3 );
  assert_eq!( client.point_count(), 0 );
}

#[tokio::test]
async fn can_push_and_flush_points_within_the_limits_of_the_devices_capacity() {
  let device = emulator::TestDevice::with_capacity( 2 );
  let ( _, mut client ) = make_emulator_and_client_with_test_device( device ).await;

  // Push four points (two more than the device's capacity)
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  assert_eq!( client.point_count(), 4 );

  let state = client.flush_points_and_wait().await.unwrap();

  // Only two points will be flushed due to the device capacity
  assert_eq!( state.points_buffered, 2 );
  assert_eq!( client.point_count(), 2 );
}

#[tokio::test]
async fn can_start_the_client() {
  let device = emulator::TestDevice::with_capacity( 2 );
  let ( mut emulator, mut client ) = make_emulator_and_client_with_test_device( device ).await;

  // Push four points (two more than the device's capacity)
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  assert_eq!( client.point_count(), 4 );

  let mut state = client.start( 1000 ).await.unwrap();

  // Only two points will be flushed immediately, due to device capacity
  assert_eq!( state.points_buffered, 2 );
  assert_eq!( client.point_count(), 2 );

  // We consume all the points from the emulator and wait. During this time:
  // (1) The client writer will auto-ping
  // (2) The client writer will realize point capacity is available
  // (3) The client writer will flush the remaining points
  emulator.consume_points( 2 );
  sleep( Duration::from_secs( 1 ) ).await;

  // Verify that all remaining points have been flushed
  client.copy_state( &mut state ).await;
  assert_eq!( state.points_buffered, 2 );
  assert_eq!( client.point_count(), 0 );
}

#[tokio::test]
async fn will_execute_a_low_watermark_callback() {
  let mut emulator = emulator::Server::start().await.unwrap();
  let ( watch_tx, mut watch_rx ) = watch::channel( 0 );

  let mut client =
    etherdream::ClientBuilder::<Point>::new( emulator.device() )
      .on_low_watermark( 2, move | count | { let _ = watch_tx.send( count ).is_ok(); })
      .connect().await
      .expect( "Failed to connect to Etherdream device" );

  // Push 3 points to the client. This engages the low watermark trigger since
  // `point_buffer = 3` (greater than our limit of `2`)
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  let _ = client.start( 1000 ).await;
  assert_eq!( watch_rx.has_changed().unwrap(), false );

  // Consume 2 points from the emulator. This will trigger the execution of the
  // low watermark callback.
  emulator.consume_points( 2 );
  assert!( verify_callback_value( &mut watch_rx, 1 ).await );

  // Verify that the low watermark callback is not executed a second time if
  // more points are consumed while in the "alert" state. A `sleep` is required
  // to allow time for the client to receive and process the ack.
  emulator.consume_points( 1 );
  sleep( Duration::from_secs( 1 ) ).await;
  assert_eq!( watch_rx.has_changed().unwrap(), false );

  // Push 3 points to the client. This re-engages the low watermark trigger.
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  client.push_point( Point::new( 0, 0, 0, 0, 0 ) );
  let _ = client.flush_points_and_wait().await;
  assert_eq!( watch_rx.has_changed().unwrap(), false );

  // Consume 3 points, verifying that the low watermark callback is executed
  // again.
  emulator.consume_points( 3 );
  assert!( verify_callback_value( &mut watch_rx, 0 ).await );
}

#[tokio::test]
async fn can_stop_the_client() {
  let ( _, mut client ) = make_emulator_and_client().await;

  let state = client.start( 1000 ).await.unwrap();
  assert_eq!( state.playback_state, etherdream::device::PlaybackState::Playing );

  let state = client.stop().await.unwrap();
  assert_eq!( state.playback_state, etherdream::device::PlaybackState::Idle );
}

#[tokio::test]
async fn can_disconnect_the_client() {
  let ( _, client ) = make_emulator_and_client().await;
  assert_eq!( client.disconnect().await, () );
}

// Creates an Etherdream emulator and client using the default test device.
async fn make_emulator_and_client() -> ( emulator::Server, etherdream::Client<Point>  ) {
  make_emulator_and_client_with_test_device( emulator::TestDevice::new() ).await
}

// Creates an Etherdream emulator and client using a provided `device`
// configuration.
async fn make_emulator_and_client_with_test_device( device: emulator::TestDevice ) -> ( emulator::Server, etherdream::Client<Point>  ) {
  let emulator = emulator::Server::start_with_device( device ).await.unwrap();

  let client =
    etherdream::ClientBuilder::<Point>::new( emulator.device() ).connect().await
      .expect( "Failed to connect to Etherdream device" );

  ( emulator, client )
}

// Waits for `watch_rx` to return `expected_value`, or times out after 1sec.
async fn verify_callback_value( watch_rx: &mut watch::Receiver<usize>, expected_value: usize ) -> bool {
  tokio::select!{
    result = watch_rx.wait_for(| count |{ *count == expected_value }) => result.is_ok(),
    _ = sleep( Duration::from_secs( 1 ) ) => false
  }
}