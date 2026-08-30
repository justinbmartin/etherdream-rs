use crate::device::{ Device, DeviceMap };

#[derive( Default )]
pub struct State {
  // The currently selected device
  device_id: Option<usize>,
  // A map of all devices
  device_map: DeviceMap
}

impl State {
  /// Get a read-only reference to the device map
  pub fn get_device_map( &self ) -> &DeviceMap { &self.device_map }
  /// Add's a device to the device map
  pub fn add_device_to_map( &mut self, device_info: etherdream::DeviceInfo ) { self.device_map.insert( device_info ) }
  /// Set the currently selected device id
  pub fn set_device( &mut self, device_id: Option<usize> ) { self.device_id = device_id }

  /// Get an immutable reference to the currently selected device
  pub fn get_device( &self ) -> Option<&Device> {
    self.device_id.and_then(|d| self.device_map.get( d ) )
  }

  /// Get a mutable reference to the currently selected device
  pub fn get_device_mut( &mut self ) -> Option<&mut Device> {
    self.device_id.and_then(|d| self.device_map.get_mut( d ) )
  }
}