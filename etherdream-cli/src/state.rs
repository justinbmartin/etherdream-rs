
use std::sync::Arc;

use tokio::sync::{ RwLock, RwLockReadGuard };

use crate::device::DeviceMap;

pub struct State {
  device_id: Option<usize>,
  device_map: Arc<RwLock<DeviceMap>>
}

impl State {
  pub fn new( device_map: Arc<RwLock<DeviceMap>> ) -> Self {
    Self{ device_id: None::<usize>, device_map }
  }

  pub fn device_id( &self ) -> Option<usize> { self.device_id }
  pub fn device_map( &self ) -> &Arc<RwLock<DeviceMap>> { &self.device_map }
}

struct ReadOnlyState {
  inner: Arc<RwLock<State>>
}

impl ReadOnlyState {
  pub fn blocking_read( &self ) -> StateReadGuard {
    StateReadGuard{ inner: self.inner.blocking_read() }
  }
}

pub struct StateReadGuard<'a> {
  inner: RwLockReadGuard<'a, State>
}

impl<'a> StateReadGuard<'a> {
  pub fn new( state: RwLockReadGuard<'a, State> ) -> Self {
    Self{ inner: state }
  }

  pub fn device_id( &self ) -> &Option<usize> { &self.inner.device_id }
  pub fn device_map( &'a self ) -> RwLockReadGuard<'a, DeviceMap> { self.inner.device_map.blocking_read() }
}