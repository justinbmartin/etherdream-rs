use std::sync::Arc;

use tokio::sync::{ RwLock, RwLockReadGuard };

use crate::device::DeviceMap;
use crate::scene;
use crate::scenes::{ self, Action };

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

pub struct State {
  device_id: Arc<RwLock<Option<usize>>>,
  device_map: Arc<RwLock<DeviceMap>>
}

impl State {
  pub fn new( device_map: Arc<RwLock<DeviceMap>> ) -> Self {
    Self{ device_id: Arc::new( RwLock::new( None::<usize> ) ), device_map }
  }

  pub fn clone_read_only_device_id( &self ) -> ReadOnly<Option<usize>> { ReadOnly::new( self.device_id.clone() ) }
  pub fn clone_read_only_device_map( &self ) -> ReadOnly<DeviceMap> { ReadOnly::new( self.device_map.clone() ) }
}

impl Clone for State {
  fn clone( &self ) -> Self {
    Self{
      device_id: self.device_id.clone(),
      device_map: self.device_map.clone()
    }
  }
}

impl scene::Actionable for State {
  type Action = Action;

  async fn invoke( &mut self, action: Action ) -> scene::SceneEvent {
    match action {
      Action::Connect( _port ) => {
        let device_id = self.device_id.read().await.unwrap();

        if let Some( device ) = self.device_map.write().await.get_mut( device_id ) {
          let _ = device.connect().await;
          return scene::SceneEvent::None;
        }

        scene::SceneEvent::Pop
      },
      Action::SelectDevice( id ) => {
        *self.device_id.write().await = Some( id );
        scene::SceneEvent::Switch( scenes::DEVICE_ID )
      },
      Action::DeselectDevice => {
        *self.device_id.write().await = None;
        scene::SceneEvent::Switch( scenes::LIST_ID )
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Read-only Wrapper

/// A read-only ARC wrapper for T's
pub struct ReadOnly<T> {
  inner: Arc<RwLock<T>>
}

impl<T> ReadOnly<T> {
  pub fn new( item: Arc<RwLock<T>> ) -> Self {
    Self{ inner: item }
  }

  // Will panic if called in an async context.
  pub fn read( &'_ self ) -> RwLockReadGuard<'_, T> {
    self.inner.blocking_read()
  }
}

impl<T> Clone for ReadOnly<T> {
  /// Creates a new ReadOnly<T> that clones the inner item.
  fn clone( &self ) -> Self {
    Self{ inner: self.inner.clone() }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scoped Device

pub struct ScopedDevice {
  device_map: ReadOnly<DeviceMap>,
  device_id: ReadOnly<Option<usize>>
}

impl ScopedDevice {
  pub fn new( state: &State ) -> Self {
    Self{ device_id: state.clone_read_only_device_id(), device_map: state.clone_read_only_device_map() }
  }

  /// Returns a scoped guard to the currently selected device.
  pub fn get( &'_ self ) -> Option<ScopedDeviceGuard<'_>> {
    self.device_id.read().map(|device_id|{
      ScopedDeviceGuard{ guard: self.device_map.read(), device_id }
    })
  }
}

impl Clone for ScopedDevice {
  fn clone( &self ) -> Self {
    Self{
      device_id: self.device_id.clone(),
      device_map: self.device_map.clone()
    }
  }
}

pub struct ScopedDeviceGuard<'a> {
  guard: RwLockReadGuard<'a, DeviceMap>,
  device_id: usize
}

impl<'a> ScopedDeviceGuard<'a> {
  pub fn info( &self ) -> &etherdream::DeviceInfo {
    self.guard.get( self.device_id ).unwrap().info()
  }

  pub fn is_connected( &self ) -> bool {
    self.guard.get( self.device_id ).unwrap().is_connected()
  }
}