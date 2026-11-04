use std::net::SocketAddr;

use etherdream::protocol;

pub struct Device {
  info: etherdream::DeviceInfo
}

impl Device {
  pub fn new( info: etherdream::DeviceInfo ) -> Self {
    Self{ info }
  }

  pub fn info( &self ) -> &etherdream::DeviceInfo { &self.info }

  pub fn address( &self ) -> SocketAddr { SocketAddr::new( self.info.ip(), protocol::CLIENT_PORT ) }
}