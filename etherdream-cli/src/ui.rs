use std::sync::Arc;

use crate::device::DeviceMap;
use crate::read_only::ReadOnlyArc;
use crate::scene::{ Actionable, SceneDefinitionContext, SceneEvent };
use crate::state::State;

use tokio::sync::{ RwLock, RwLockReadGuard };

mod connect;
mod list;
mod device;

pub const CONNECT_ID: &str  = "connect";
pub const DEVICE_ID: &str   = "device";
pub const LIST_ID: &str     = "list";

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Actions

#[derive( Debug )]
pub enum Action {
  Connect( u16 ),
  SelectDevice( usize ),
  DeselectDevice
}

impl Actionable for State {
  type Action = Action;

  async fn invoke( &mut self, action: Action ) -> SceneEvent {
    match action {
      Action::Connect( _port ) => {
        let device_id = self.device_id().unwrap();

        if let Some( device ) = self.device_map().write().await.get_mut( device_id ) {
          let _ = device.connect().await;
          return SceneEvent::None;
        }

        SceneEvent::Pop
      },
      Action::SelectDevice( device_id ) => {
        self.set_device_id( Some( device_id ) );
        SceneEvent::Switch( DEVICE_ID )
      },
      Action::DeselectDevice => {
        self.set_device_id( None );
        SceneEvent::Switch( LIST_ID )
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene Definitions

pub fn build_scenes( ctx: &mut SceneDefinitionContext<State> ) {
  ctx.add_scene( LIST_ID, Box::new( list::ListScene::new( ctx.state().clone() ) ) );
  ctx.add_scene( DEVICE_ID, Box::new( device::DeviceScene::new( ctx.state().clone() ) ) );
  ctx.add_scene( CONNECT_ID, Box::new( connect::ConnectScene::new( ctx.state().clone() ) ) );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Read-only Device

pub struct ReadOnlyDevice {
  state: Arc<RwLock<State>>
}

impl ReadOnlyDevice {
  fn new( state: Arc<RwLock<State>> ) -> Self {
    Self{ state }
  }

  /// Returns a guard to the currently selected device.
  pub fn info( &'_ self ) -> Option<etherdream::DeviceInfo> {
    let state = self.state.blocking_read();
    if let Some( device_id ) = state.device_id() {
      if let Some( device ) = state.device_map().blocking_read().get( device_id ) {
        return Some( device.info().clone() );
      }
    }

    None
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