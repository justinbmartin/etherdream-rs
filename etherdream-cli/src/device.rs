use std::collections::{ HashMap, hash_map::Iter };
use std::net::SocketAddr;

use etherdream::protocol;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device

pub struct Device {
  info: etherdream::DeviceInfo,
  client: Option<etherdream::Client>,
  generator: Option<etherdream::Generator>
}

impl Device {
  /// Called from `DeviceMap` to create a new `Device`.
  fn new( info: etherdream::DeviceInfo ) -> Self {
    Self{
      client: None,
      info,
      generator: None
    }
  }

  /// ...
  pub fn address( &self ) -> SocketAddr { SocketAddr::new( self.info.ip(), protocol::CLIENT_PORT ) }

  /// Returns true if the device is connected.
  pub fn is_connected( &self ) -> bool { self.client.is_some() || self.generator.is_some() }

  //
  pub async fn connect( &mut self ) -> Result<(), etherdream::client::Error> {
    if self.generator.is_some() { return Ok( () ); }

    match etherdream::connect( self.info ).await {
      Ok( client ) => {
        self.client = Some( client );
        Ok( () )
      }
      Err( err ) => Err( err )
    }
  }

  //
  pub async fn disconnect( &mut self ) {
    if let Some( generator ) = self.generator.take() && let Ok( client ) = generator.into_client().await {
      client.disconnect().await
    }
  }

  /// Returns a reference to the active device generator, if one is set.
  pub fn generator( &self ) -> Option<&etherdream::Generator> { self.generator.as_ref() }

  /// Creates and starts a generator from `executable`.
  pub async fn generate( &mut self, executable: Box<dyn etherdream::generator::Executable> ) {
    if let Some( client ) = self.client.take() {
      let mut generator = etherdream::make_generator( client, executable );
      generator.start().await;

      self.generator = Some( generator );
    }
  }

  pub fn info( &self ) -> &etherdream::DeviceInfo {
    &self.info
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device Map

#[derive( Default )]
pub struct DeviceMap {
  inner: HashMap<SocketAddr,Device>,
  version: usize
}

impl DeviceMap{
  /// Returns an immutable reference to a device.
  pub fn get( &self, address: &SocketAddr ) -> Option<&Device> { self.inner.get( address ) }
  pub fn get_mut( &mut self, address: &SocketAddr ) -> Option<&mut Device> { self.inner.get_mut( address ) }

  /// Inserts a new device into the map, incrementing the map version.
  pub fn insert( &mut self, address: SocketAddr, info: etherdream::DeviceInfo ) {
    self.inner.insert( address, Device::new( info ) );
    self.version = self.version.saturating_add( 1 );
  }

  /// Returns an iterator of devices.
  pub fn iter( &self ) -> Iter<'_, SocketAddr, Device> {
    self.inner.iter()
  }

  /// Returns the number of devices in the map.
  pub fn len( &self ) -> usize {
    self.inner.len()
  }

  /// Returns the current version of the map.
  pub fn version( &self ) -> usize {
    self.version
  }
}