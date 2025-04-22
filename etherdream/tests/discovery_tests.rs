use std::net::{ IpAddr, Ipv4Addr, SocketAddr };
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::net::UdpSocket;

use etherdream::{ device, discovery };
use zerocopy::IntoBytes;

// Shared mutex to force serial execution of discovery tests, where required
static DISCOVERY_MUTEX: std::sync::Mutex<u8> = std::sync::Mutex::new( 0 );

// Returns a device builder with default values for testing
fn make_default_device_builder_for_test() -> device::DeviceBuilder {
  return device::DeviceBuilder::new()
    .buffer_capacity( 1024 )
    .light_engine_state( device::LightEngineState::Ready )
    .mac_address([ 0, 1, 2, 3, 4, 5 ])
    .max_points_per_second( 128 )
    .playback_state( device::PlaybackState::Idle )
    .points_lifetime( 16384 )
    .points_per_second( 128 )
    .source( device::Source::Ilda )
    .version( 0, 1 )
    .to_owned();
}

// Sends `count` broadcast message's for `device`
async fn send_etherdream_broadcasts( device: &device::Device, count: usize ) {
  let local_address = SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 );
  let destination_address = SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), discovery::BROADCAST_PORT );

  let local_socket = UdpSocket::bind( local_address ).await.expect( "Failed to bind to localhost for testing." );
  local_socket.set_broadcast( true ).expect( "Failed to set socket to broadcast for testing." );

  for _ in 0..count {
    if let Err(_) = local_socket.send_to( device.as_bytes(), destination_address ).await {
      panic!( "Failed to send broadcast device for testing." )
    }
  }
}

#[tokio::test]
async fn receives_an_etherdream_broadcast() {
  let _lock = DISCOVERY_MUTEX.lock();

  //
  let devices = Arc::new( RwLock::new( Vec::new() ) );
  let test_device = make_default_device_builder_for_test().to_device();

  let discovery_handle = 
    tokio::task::spawn({ 
      let devices = devices.clone();

      async move {
        return discovery::Server::new()
          .limit( 1 )
          .serve( move | address, device |{
            devices.write().push( ( *address, *device ) );
          }).await;
      }
    });

  send_etherdream_broadcasts( &test_device, 1 ).await;
  let _ = discovery_handle.await.unwrap();

  // There should only be a single device
  assert_eq!( devices.read().len(), 1 );

  // Test the device properties received
  let &( address, device ) = devices.read().get( 0 ).unwrap();

  assert_eq!( address.ip(), IpAddr::V4( Ipv4Addr::new( 127, 0, 0, 1 ) ) );

  assert!( device.is_light_engine_state( device::LightEngineState::Ready ) );
  assert!( device.is_playback_state( device::PlaybackState::Idle ) );
  assert!( device.is_source( device::Source::Ilda ) );

  assert_eq!( device.buffer_capacity(), 1024 );
  assert_eq!( device.light_engine_state(), device::LightEngineState::Ready );
  assert_eq!( device.mac_address(), MacAddress::new([ 0, 1 , 2, 3, 4, 5 ]) );
  assert_eq!( device.max_points_per_second(), 128 );
  assert_eq!( device.playback_state(), device::PlaybackState::Idle );
  assert_eq!( device.points_lifetime(), 16384 );
  assert_eq!( device.points_per_second(), 128 );
  assert_eq!( device.source(), device::Source::Ilda );
  assert_eq!( device.version(), device::Version::new( 0, 1 ) );
}

#[tokio::test]
async fn will_receive_one_device_if_a_limit_is_defined() {
  let _lock = DISCOVERY_MUTEX.lock();

  let devices = Arc::new( RwLock::new( Vec::new() ) );

  let default_device_builder = make_default_device_builder_for_test();
  let device_10 = default_device_builder.clone().mac_address([ 10;6 ]).to_device();
  let device_20 = default_device_builder.clone().mac_address([ 20;6 ]).to_device();

  let discovery_handle = 
    tokio::task::spawn({ 
      let devices = devices.clone();

      async move {
        return discovery::Server::new()
          .limit( 1 )
          .serve( move | _address, device |{
            devices.write().push( *device );
          }).await;
      }
    });

  send_etherdream_broadcasts( &device_10, 1 ).await;
  send_etherdream_broadcasts( &device_20, 1 ).await;
  let _ = discovery_handle.await.unwrap();

  // There should only be a single device
  assert_eq!( devices.read().len(), 1 );
  assert_eq!( devices.read().get( 0 ).unwrap().mac_address(), MacAddress::new([10;6]) );
}

#[tokio::test]
async fn will_execute_the_user_provided_callback_once_for_each_unique_device() {
  let _lock = DISCOVERY_MUTEX.lock();

  let devices = Arc::new( RwLock::new( Vec::new() ) );

  let default_device_builder = make_default_device_builder_for_test();
  let device_10 = default_device_builder.clone().mac_address([ 10;6 ]).to_device();
  let device_20 = default_device_builder.clone().mac_address([ 20;6 ]).to_device();

  let discovery_handle = 
    tokio::task::spawn({ 
      let devices = devices.clone();

      async move {
        return discovery::Server::new()
          .limit( 2 )
          .serve( move | _address, device |{
            devices.write().push( *device );
          }).await;
      }
    });

  // Sending `device_10` ten times and `device_20` once
  send_etherdream_broadcasts( &device_10, 10 ).await;
  send_etherdream_broadcasts( &device_20, 1 ).await;
  let _ = discovery_handle.await.unwrap();

  // There should only be a two devices
  assert_eq!( devices.read().len(), 2 );
  assert_eq!( devices.read().get( 0 ).unwrap().mac_address(), [10;6] );
  assert_eq!( devices.read().get( 1 ).unwrap().mac_address(), [20;6] );
}