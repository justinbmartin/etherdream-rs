use std::sync::Arc;

use tokio::sync::RwLock;

use crate::device::{ Device, DeviceMap };
use crate::scene;

mod common;
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

impl scene::Actionable for State {
  type Action = Action;

  async fn invoke( &mut self, action: Action ) -> scene::SceneEvent {
    match action {
      Action::Connect( _port ) => {
        if let Some( device ) = self.get_device_mut() {
          let _ = device.connect().await;
          return scene::SceneEvent::None;
        }

        scene::SceneEvent::Pop
      },
      Action::SelectDevice( device_id ) => {
        self.set_device( Some( device_id ) );
        scene::SceneEvent::Switch( DEVICE_ID )
      },
      Action::DeselectDevice => {
        self.set_device( None );
        scene::SceneEvent::Switch( LIST_ID )
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene Definitions

pub fn build_scenes( state: Arc<RwLock<State>> ) -> Result<( scene::Foreground<State>, scene::Background<State> ),scene::BuilderError> {
  let mut builder = scene::Builder::new( state );

  builder.add_scene( LIST_ID, Box::new( list::ListScene::new() ) );
  builder.add_scene( DEVICE_ID, Box::new( device::DeviceScene::default() ) );
  builder.add_scene( CONNECT_ID, Box::new( connect::ConnectScene::new() ) );

  builder.build()
}