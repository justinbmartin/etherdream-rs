use std::net::SocketAddr;
use std::sync::Arc;

use etherdream::discovery;
use tokio::sync::RwLock;

use crate::device::{ Device, DeviceMap };
use crate::scene;

mod common;
mod list;
mod generator;

pub const GENERATOR_ID: &str  = "generator";
pub const LIST_ID: &str       = "list";

#[derive( Debug )]
pub enum Action {
  Connect,
  DeselectDevice,
  Disconnect,
  SelectDevice( SocketAddr )
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

pub struct State {
  // The currently selected device.
  current_device_id: Option<SocketAddr>,
  // Map of instantiated Etherdream devices.
  devices: DeviceMap,
  // The device registry shared with the discovery service.
  registry: discovery::Registry
}

impl State {
  /// Creates a new `State` instance.
  pub fn new( registry: discovery::Registry ) -> Self {
    Self{
      current_device_id: None,
      devices: DeviceMap::default(),
      registry
    }
  }

  /// Returns an immutable reference to the `State`s registered devices.
  pub fn get_devices( &self ) -> &DeviceMap { &self.devices }

  /// Registers a device from the registry into the `DeviceMap` by `address`.
  pub async fn register_device( &mut self, address: SocketAddr ) {
    if let Some( broadcast ) = self.registry.read().await.get( &address ) {
      self.devices.insert( address, *broadcast.device_info() );
    }
  }
  /// Set the currently selected device id
  pub fn set_current_device( &mut self, device_id: Option<SocketAddr> ) {
    self.current_device_id = device_id
  }

  /// Get an immutable reference to the currently selected device
  pub fn get_current_device( &self ) -> Option<&Device> {
    self.current_device_id.and_then(|d| self.devices.get( &d ) )
  }

  /// Get a mutable reference to the currently selected device
  pub fn get_current_device_mut( &mut self ) -> Option<&mut Device> {
    self.current_device_id.and_then(|d| self.devices.get_mut( &d ) )
  }
}

impl scene::Actionable for State {
  type Action = Action;

  async fn invoke( &mut self, action: Action ) -> scene::SceneEvent {
    match action {
      Action::Connect => {
        if let Some( device ) = self.get_current_device_mut() {
          let _ = device.connect().await;
        }

        scene::SceneEvent::None
      }
      Action::DeselectDevice => {
        self.set_current_device( None );
        scene::SceneEvent::Switch( LIST_ID )
      }
      Action::Disconnect => {
        if let Some( device ) = self.get_current_device_mut() {
          let _ = device.disconnect().await;
        }

        scene::SceneEvent::None
      }
      Action::SelectDevice( device_id ) => {
        self.set_current_device( Some( device_id ) );
        scene::SceneEvent::Switch( GENERATOR_ID )
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene Definitions

pub fn build_scenes( state: Arc<RwLock<State>> ) -> Result<( scene::Foreground<State>, scene::Background<State> ),scene::BuilderError> {
  let mut builder = scene::Builder::new( state );

  builder.add_scene( LIST_ID, Box::new( list::ListScene::new() ) );
  builder.add_scene( GENERATOR_ID, Box::new( generator::GeneratorScene::default() ) );
  builder.build()
}