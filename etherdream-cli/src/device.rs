use std::collections::{ HashMap, hash_map::Iter };
use std::net::SocketAddr;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device

pub struct Device {
  info: etherdream::DeviceInfo,
  generator: Option<etherdream::Generator>
}

impl Device {
  fn new( info: etherdream::DeviceInfo ) -> Self {
    Self{
      info,
      generator: None
    }
  }

  pub fn address( &self ) -> &SocketAddr {
    self.info.address()
  }

  pub fn generator( &self ) -> &Option<etherdream::Generator> {
    &self.generator
  }

  pub fn set_generator( &mut self, generator: etherdream::Generator ) {
    self.generator = Some( generator );
  }

  pub fn info( &self ) -> &etherdream::DeviceInfo {
    &self.info
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device Map

pub struct DeviceMap {
  data: HashMap<SocketAddr,Device>,
  version: usize
}

impl Default for DeviceMap {
  fn default() -> Self {
    Self{
      data: HashMap::new(),
      version: 0
    }
  }
}

impl DeviceMap{
  pub fn get( &self, addr: &SocketAddr ) -> Option<&Device> {
    self.data.get( addr )
  }

  pub fn get_mut( &mut self, addr: &SocketAddr ) -> Option<&mut Device> {
    self.data.get_mut( addr )
  }

  pub fn insert( &mut self, info: etherdream::DeviceInfo ) {
    self.data.insert( *info.address(), Device::new( info ) );
    self.version += 1;
  }

  pub fn iter( &self ) -> Iter<'_, SocketAddr, Device> {
    self.data.iter()
  }

  pub fn len( &self ) -> usize {
    self.data.len()
  }

  pub fn version( &self ) -> usize {
    self.version
  }
}