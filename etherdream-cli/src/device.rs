use std::collections::{ HashMap, hash_map::Iter };

use crate::executors::Noop;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device

pub struct Device {
  id: usize,
  info: etherdream::DeviceInfo,
  client: Option<etherdream::Client>,
  generator: Option<etherdream::Generator>
}

impl Device {
  /// Called from `DeviceMap` to create a new `Device`.
  fn new( id: usize, info: etherdream::DeviceInfo ) -> Self {
    Self{
      client: None,
      id,
      info,
      generator: None
    }
  }

  /// Returns the id of the device
  pub fn id( &self ) -> usize { self.id }

  /// Returns true if the device is connected.
  pub fn is_connected( &self ) -> bool { self.client.is_some() || self.generator.is_some() }

  //
  pub async fn connect( &mut self ) -> Result<(), etherdream::client::Error> {
    if self.generator.is_some() { return Ok( () ); }

    etherdream::connect( self.info ).await
      .and_then(| client |{
        self.client = Some( client );
        Ok( () )
      })
  }

  //
  pub async fn disconnect( &mut self ) {
    if let Some( generator ) = self.generator.take() && let Ok( client ) = generator.into_client().await {
      client.disconnect().await
    }
  }

  /// Returns a reference to the active device generator, if one is set.
  pub fn generator( &self ) -> Option<&etherdream::Generator> { self.generator.as_ref() }

  /// ...
  pub fn set_generator( &mut self, generator: etherdream::Generator ) {
    self.generator = Some( generator );
  }

  pub fn take_generator( &mut self ) -> Option<etherdream::Generator> {
    self.generator.take()
  }

  pub fn info( &self ) -> &etherdream::DeviceInfo {
    &self.info
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device Map

pub struct DeviceMap {
  inner: HashMap<usize,Device>,
  version: usize
}

impl Default for DeviceMap {
  fn default() -> Self {
    Self{
      inner: HashMap::new(),
      version: 0
    }
  }
}

impl DeviceMap{
  /// Returns an immutable reference to a device.
  pub fn get( &self, id: usize ) -> Option<&Device> { self.inner.get( &id ) }

  /// Returns a mutable reference to a device.
  pub fn get_mut( &mut self, id: usize ) -> Option<&mut Device> { self.inner.get_mut( &id ) }

  /// Inserts a new device into the map, incrementing the map version.
  pub fn insert( &mut self, info: etherdream::DeviceInfo ) {
    let id = self.version;
    self.inner.insert( id, Device::new( id, info ) );
    self.version = self.version.saturating_add( 1 );
  }

  /// Returns an iterator of devices.
  pub fn iter( &self ) -> Iter<'_, usize, Device> {
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