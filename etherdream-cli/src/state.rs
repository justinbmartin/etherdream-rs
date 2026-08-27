use crate::device::{ Device, DeviceMap };

#[derive( Default )]
pub struct State {
  device_id: Option<usize>,
  device_map: DeviceMap
}

impl State {
  // Device Id
  pub fn device_id( &self ) -> Option<usize> { self.device_id }
  pub fn set_device_id( &mut self, device_id: Option<usize> ) { self.device_id = device_id }

  // Device Map
  pub fn device_by_id( &self, device_id: usize ) -> Option<&Device> { self.device_map.get( device_id ) }
  pub fn device_by_id_mut( &mut self, device_id: usize ) -> Option<&mut Device> { self.device_map.get_mut( device_id ) }
  pub fn add_device_to_map( &mut self, device_info: etherdream::DeviceInfo ) { self.device_map.insert( device_info ) }
  pub fn device_map( &self ) -> &DeviceMap { &self.device_map }

  //
  pub fn current_device( &self ) -> Option<&Device> {
    if let Some( device_id ) = self.device_id {
      self.device_map.get( device_id )
    } else {
      None
    }
  }
}