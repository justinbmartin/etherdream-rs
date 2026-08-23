use std::sync::Arc;

use tokio::sync::{ RwLock, RwLockReadGuard };

use crate::device::DeviceMap;
use crate::read_only::ReadOnly;
use crate::scene::{ Actionable, SceneDefinitionContext, SceneEvent };

mod connect;
mod list;
mod device;

pub const CONNECT_ID: &str  = "connect";
pub const DEVICE_ID: &str   = "device";
pub const LIST_ID: &str     = "list";

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
        SceneEvent::Switch( DEVICE_ID )
      },
      Action::DeselectDevice => {
        *self.device_id.write().await = None;
        SceneEvent::Switch( LIST_ID )
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Read-only Device

pub struct ReadOnlyDevice {
  device_map: ReadOnly<DeviceMap>,
  device_id: ReadOnly<Option<usize>>
}

impl ReadOnlyDevice {
  pub fn new( state: &State ) -> Self {
    Self{ device_id: state.clone_read_only_device_id(), device_map: state.clone_read_only_device_map() }
  }

  /// Returns a guard to the currently selected device.
  pub fn get( &'_ self ) -> Option<ReadOnlyDeviceGuard<'_>> {
    self.device_id.blocking_read().map(|device_id|{
      ReadOnlyDeviceGuard{ guard: self.device_map.blocking_read(), device_id }
    })
  }
}

impl Clone for ReadOnlyDevice {
  fn clone( &self ) -> Self {
    Self{
      device_id: self.device_id.clone(),
      device_map: self.device_map.clone()
    }
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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

pub fn build_scenes( ctx: &mut SceneDefinitionContext<State> ) {
  let device = ReadOnlyDevice::new( ctx.state() );

  ctx.add_scene( LIST_ID, Box::new( list::ListScene::new( ctx.state().clone_read_only_device_map() ) ) );
  ctx.add_scene( DEVICE_ID, Box::new( device::DeviceScene::new( device.clone() ) ) );
  ctx.add_scene( CONNECT_ID, Box::new( connect::ConnectScene::new( device.clone() ) ) );
}