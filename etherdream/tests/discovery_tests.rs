use std::io;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::net::UdpSocket;
use tokio::time::Duration;

use etherdream::{ device::*, discovery };

mod support;
use support::DeviceBuilder;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Test Helpers

// Starts a discovery server with `limit`. Returns a tuple containing (1) the
// shared vector that discovered device(s) will be persisted into via callback, 
// and (2) the connection for the server.
async fn setup_discovery( limit: usize ) -> ( Arc<RwLock<Vec<( SocketAddr, Device )>>>, discovery::Connection ) {
  let callback_devices = Arc::new( RwLock::new( Vec::new() ) );
  let callback_devices_1 = callback_devices.clone();

  let server = 
    discovery::Server::new( move | address, device | { callback_devices_1.write().push( ( address, device ) ); })
    .address( SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 ) )
    .duration( Duration::from_secs( 5 ) )
    .limit( limit )
    .serve();

  if let Ok( conn ) = server.await {
    return ( callback_devices, conn );
  } else {
    panic!( "Failed to create discovery server." )
  }
}

// Sends `count` broadcast message's for `device`
async fn send_etherdream_broadcasts( address: SocketAddr, device: Device, count: usize ) -> io::Result<()> {
  let local_address = SocketAddr::new( IpAddr::V4( Ipv4Addr::LOCALHOST ), 0 );

  let local_socket = UdpSocket::bind( local_address ).await?;
  local_socket.set_broadcast( true )?;

  for _ in 0..count {
    if let Err( err ) = local_socket.send_to( &device.to_bytes(), address ).await {
      panic!( "Failed to broadcast device: {err}" );
    }
  }

  return Ok(());
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Unit Tests

#[tokio::test]
async fn receives_an_etherdream_broadcast() -> io::Result<()> {
  let ( callback_devices, conn ) = setup_discovery( 1 ).await;

  // Send a test device broadcast
  let test_device = DeviceBuilder::default().to_device();
  send_etherdream_broadcasts( conn.address(), test_device, 1 ).await?;

  // Let the discovery server run to completion and verify the discovered devices
  let discovered_devices = conn.join().await?;
  assert_eq!( discovered_devices.len(), 1 );

  let discovered_device = discovered_devices.values().next().unwrap();
  assert_eq!( discovered_device.mac_address, test_device.mac_address );

  // Validate the device(s) we recorded via the run-time callback
  let callback_devices = callback_devices.read();

  assert_eq!( callback_devices.len(), 1 );
  let &( address, callback_device ) = callback_devices.get( 0 ).unwrap();

  assert_eq!( address.ip(), IpAddr::V4( Ipv4Addr::LOCALHOST ) );

  assert_eq!( callback_device.buffer_capacity, 1024 );
  assert_eq!( callback_device.mac_address, MacAddress::new([ 0, 1 , 2, 3, 4, 5 ]) );
  assert_eq!( callback_device.max_points_per_second, 128 );
  assert_eq!( callback_device.version, Version::new( 0, 1 ) );

  assert_eq!( callback_device.state.light_engine_state, LightEngineState::Ready );
  assert_eq!( callback_device.state.playback_state, PlaybackState::Idle );
  assert_eq!( callback_device.state.points_lifetime, 16384 );
  assert_eq!( callback_device.state.points_per_second, 128 );
  assert_eq!( callback_device.state.source, Source::Ilda );
  
  return Ok(());
}

#[tokio::test]
async fn will_receive_a_limited_number_of_devices() -> io::Result<()> {
  let ( callback_devices, conn ) = setup_discovery( 1 ).await;

  let device_builder = DeviceBuilder::default();
  let device_10 = device_builder.mac_address([10; 6]).to_device();
  let device_20 = device_builder.mac_address([20; 6]).to_device();

  send_etherdream_broadcasts( conn.address(), device_10, 1 ).await?;
  send_etherdream_broadcasts( conn.address(), device_20, 1 ).await?;
  
  //
  let _ = conn.join().await?;

  // There should only be a single device for MAC=`10::10::10::10::10::10`
  let callback_devices = callback_devices.read();

  assert_eq!( callback_devices.len(), 1 );
  assert_eq!( callback_devices.get( 0 ).unwrap().1.mac_address, device_10.mac_address );
  return Ok(());
}

#[tokio::test]
async fn will_execute_the_user_provided_callback_once_for_each_unique_device() -> io::Result<()> {
  let ( callback_devices, conn ) = setup_discovery( 2 ).await;

  let device_builder = DeviceBuilder::default();
  let device_10 = device_builder.mac_address([10; 6]).to_device();
  let device_20 = device_builder.mac_address([20; 6]).to_device();

  // Sending `device_10` ten times and `device_20` once
  send_etherdream_broadcasts( conn.address(), device_10, 10 ).await?;
  send_etherdream_broadcasts( conn.address(), device_20, 1 ).await?;
  
  // ...
  let _ = conn.join().await?;

  // There should only be a two devices
  let callback_devices = callback_devices.read();

  assert_eq!( callback_devices.len(), 2 );
  assert_eq!( callback_devices.get( 0 ).unwrap().1.mac_address, device_10.mac_address );
  assert_eq!( callback_devices.get( 1 ).unwrap().1.mac_address, device_20.mac_address );
  return Ok(());
}