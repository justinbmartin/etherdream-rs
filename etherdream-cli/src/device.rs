pub struct Device {
  info: etherdream::DeviceInfo,
  generator: Option<etherdream::Generator>
}

impl Device {
  pub fn new( info: etherdream::DeviceInfo ) -> Self {
    Self{
      info,
      generator: None
    }
  }

  pub fn info( &self ) -> &etherdream::DeviceInfo {
    &self.info
  }
}