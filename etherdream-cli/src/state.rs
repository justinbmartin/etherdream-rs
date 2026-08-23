use std::sync::Arc;

use tokio::sync::{ RwLock, RwLockReadGuard };

use crate::action::Action;
use crate::device::DeviceMap;
use crate::read_only::ReadOnlyArc;
use crate::scene::{ Actionable, SceneEvent };
use crate::ui;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

pub struct State {
  device_id: Arc<RwLock<Option<usize>>>,
  device_map: Arc<RwLock<DeviceMap>>
}

impl State {
  pub fn new( device_map: Arc<RwLock<DeviceMap>> ) -> Self {
    Self{ device_id: Arc::new( RwLock::new( None::<usize> ) ), device_map }
  }

  pub fn read_only_device_map( &self ) -> ReadOnlyArc<DeviceMap> { ReadOnlyArc::new( self.device_map.clone() ) }
}

impl Clone for State {
  fn clone( &self ) -> Self {
    Self{
      device_id: self.device_id.clone(),
      device_map: self.device_map.clone()
    }
  }
}

impl Actionable for State {
  type Action = Action;

  async fn invoke( &mut self, action: Action ) -> SceneEvent {
    match action {
      Action::Connect( _port ) => {
        let device_id = self.device_id.read().await.unwrap();

        if let Some( device ) = self.device_map.write().await.get_mut( device_id ) {
          let _ = device.connect().await;
          return SceneEvent::None;
        }

        SceneEvent::Pop
      },
      Action::SelectDevice( id ) => {
        *self.device_id.write().await = Some( id );
        SceneEvent::Switch( ui::DEVICE_ID )
      },
      Action::DeselectDevice => {
        *self.device_id.write().await = None;
        SceneEvent::Switch( ui::LIST_ID )
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Read-only Device

pub struct ReadOnlyDevice {
  state: State
}

impl ReadOnlyDevice {
  pub fn new( state: State ) -> Self {
    Self{ state }
  }

  /// Returns a guard to the currently selected device.
  pub fn get( &'_ self ) -> Option<ReadOnlyDeviceGuard<'_>> {
    self.state.device_id.blocking_read().map(|device_id|{
      ReadOnlyDeviceGuard{ guard: self.state.device_map.blocking_read(), device_id }
    })
  }
}

impl Clone for ReadOnlyDevice {
  fn clone( &self ) -> Self {
    Self{ state: self.state.clone() }
  }
}

pub struct ReadOnlyDeviceGuard<'a> {
  guard: RwLockReadGuard<'a, DeviceMap>,
  device_id: usize
}

impl<'a> ReadOnlyDeviceGuard<'a> {
  pub fn info( &self ) -> &etherdream::DeviceInfo {
    self.guard.get( self.device_id ).unwrap().info()
  }

  pub fn is_connected( &self ) -> bool {
    self.guard.get( self.device_id ).unwrap().is_connected()
  }
}