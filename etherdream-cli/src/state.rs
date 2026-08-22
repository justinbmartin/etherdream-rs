use std::sync::Arc;

use tokio::sync::{ RwLock, RwLockReadGuard };

use crate::device::DeviceMap;
use crate::scene;
use crate::scenes;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

#[derive( Debug )]
pub enum Action {
  Connect( u16 ),
  SelectDevice( usize ),
  DeselectDevice
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

pub struct State {
  device_id: Arc<RwLock<Option<usize>>>,
  device_map: Arc<RwLock<DeviceMap>>
}

impl State {
  pub fn new( device_map: Arc<RwLock<DeviceMap>> ) -> Self {
    Self{ device_id: Arc::new( RwLock::new( None::<usize> ) ), device_map }
  }

  pub fn device_id( &self ) -> ReadOnly<Option<usize>> { ReadOnly::new( self.device_id.clone() ) }
  pub fn device_map( &self ) -> ReadOnly<DeviceMap> { ReadOnly::new( self.device_map.clone() ) }
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

pub struct ReadOnly<T> {
  inner: Arc<RwLock<T>>
}

impl<T> ReadOnly<T> {
  pub fn new( item: Arc<RwLock<T>> ) -> Self {
    Self{ inner: item }
  }

  pub fn read( &'_ self ) -> RwLockReadGuard<'_, T> {
    self.inner.blocking_read()
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scoped Device

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device Guard

pub struct DeviceGuard<'a> {
  guard: RwLockReadGuard<'a, DeviceMap>,
  device_id: usize
}

impl<'a> DeviceGuard<'a> {
  pub fn info( &self ) -> &etherdream::DeviceInfo {
    self.guard.get( self.device_id ).unwrap().info()
  }

  pub fn is_connected( &self ) -> bool {
    self.guard.get( self.device_id ).unwrap().is_connected()
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scoped Device

pub struct ScopedDevice {
  device_map: ReadOnly<DeviceMap>,
  device_id: ReadOnly<Option<usize>>
}

impl ScopedDevice {
  pub fn new( device_id: ReadOnly<Option<usize>>, device_map: ReadOnly<DeviceMap> ) -> Self {
    Self{ device_id, device_map }
  }

  pub fn get( &'_ self ) -> Option<DeviceGuard<'_>> {
    match *self.device_id.read() {
      Some( device_id ) => {
        let guard = self.device_map.read();
        Some( DeviceGuard{ guard, device_id })
      }
      None => None
    }
  }
}