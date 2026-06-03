use std::collections::{ HashMap, hash_map::Iter };
use std::net::{ IpAddr, SocketAddr };

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device

pub struct Device {
  info: etherdream::DeviceInfo,
  generator: Option<etherdream::Generator>
}

impl Device {
  /// Creates a new `Device` using an `etherdream::DeviceInfo`
  fn new( info: etherdream::DeviceInfo ) -> Self {
    Self{
      info,
      generator: None
    }
  }

  pub fn ip( &self ) -> IpAddr {
    self.info.ip()
  }

  pub fn generator( &self ) -> &Option<etherdream::Generator> {
    &self.generator
  }

  pub fn set_generator( &mut self, generator: etherdream::Generator ) {
    self.generator = Some( generator );
  }

  pub fn take_generator( &mut self ) -> Option<etherdream::Generator> {
    self.generator.take()
  }

  pub fn disconnect( &mut self ) {
    self.generator = None
  }

  pub fn info( &self ) -> &etherdream::DeviceInfo {
    &self.info
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device Map

pub struct DeviceMap {
  inner: HashMap<SocketAddr,Device>,
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
  pub fn get( &self, addr: &SocketAddr ) -> Option<&Device> {
    self.inner.get( addr )
  }

  /// Returns a mutable reference to a device.
  pub fn get_mut( &mut self, addr: &SocketAddr ) -> Option<&mut Device> {
    self.inner.get_mut( addr )
  }

  /// Inserts a new device into the map, incrementing the map version.
  pub fn insert( &mut self, info: etherdream::DeviceInfo ) {
    self.inner.insert( *info.broadcast_address(), Device::new( info ) );
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