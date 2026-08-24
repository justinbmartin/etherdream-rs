
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::device::DeviceMap;

pub struct State {
  device_id: Arc<RwLock<Option<usize>>>,
  device_map: Arc<RwLock<DeviceMap>>
}

impl State {
  pub fn new( device_map: Arc<RwLock<DeviceMap>> ) -> Self {
    Self{ device_id: Arc::new( RwLock::new( None::<usize> ) ), device_map }
  }

  pub fn device_id( &self ) -> &Arc<RwLock<Option<usize>>> { &self.device_id }
  pub fn device_map( &self ) -> &Arc<RwLock<DeviceMap>> { &self.device_map }
}

impl Clone for State {
  fn clone( &self ) -> Self {
    Self{
      device_id: self.device_id.clone(),
      device_map: self.device_map.clone()
    }
  }
}