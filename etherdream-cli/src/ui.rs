use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use etherdream::discovery;
use tokio::sync::RwLock;

use crate::scene;
use crate::sequencer::Sequencer;

mod common;
mod device;
mod list;

#[derive( Debug )]
pub enum Action {
  Back,
  Connect,
  Disconnect,
  Play( &'static str ),
  SelectDevice( SocketAddr ),
  Stop
}

#[derive( Clone, Copy, Debug, Eq, Hash, PartialEq )]
pub enum SceneId {
  ListDevices,
  ViewDevice
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

pub struct State {
  // The currently selected device.
  current_device_id: Option<SocketAddr>,
  // Map of instantiated Etherdream devices.
  devices: HashMap<SocketAddr,Sequencer>,
  //
  devices_version: usize,
  // The device registry shared with the discovery service.
  registry: discovery::Registry
}

impl State {
  /// Creates a new `State` instance.
  pub fn new( registry: discovery::Registry ) -> Self {
    Self{
      current_device_id: None,
      devices: HashMap::default(),
      devices_version: 0,
      registry
    }
  }

  /// Returns an immutable reference to the `State`s registered devices.
  pub fn devices( &self ) -> &HashMap<SocketAddr,Sequencer> { &self.devices }

  ///
  pub fn devices_version( &self ) -> usize { self.devices_version }

  /// Registers a device from the registry into the `DeviceMap` by `address`.
  pub async fn register_device( &mut self, address: SocketAddr ) {
    if let Some( broadcast ) = self.registry.read().await.get( &address ) {
      self.devices.insert( address, Sequencer::new( *broadcast.device_info() ) );
      self.devices_version = self.devices_version.saturating_add( 1 );
    }
  }
  /// Set the currently selected device id
  pub fn set_current_device( &mut self, device_id: Option<SocketAddr> ) {
    self.current_device_id = device_id
  }

  /// Get an immutable reference to the currently selected device
  pub fn get_current_device( &self ) -> Option<&Sequencer> {
    self.current_device_id.and_then(|d| self.devices.get( &d ) )
  }

  /// Get a mutable reference to the currently selected device
  pub fn get_current_device_mut( &mut self ) -> Option<&mut Sequencer> {
    self.current_device_id.and_then(|d| self.devices.get_mut( &d ) )
  }
}

impl scene::SceneController for State {
  type Action = Action;
  type SceneId = SceneId;

  async fn invoke( &mut self, action: Action ) -> scene::SceneEvent<Self::SceneId> {
    match action {
      Action::Back => {
        scene::SceneEvent::Pop
      }
      Action::Connect => {
        if let Some( device ) = self.get_current_device_mut() {
          let _ = device.connect().await;
        }

        scene::SceneEvent::None
      }
      Action::Disconnect => {
        if let Some( device ) = self.get_current_device_mut() {
          let _ = device.disconnect().await;
        }

        scene::SceneEvent::None
      }
      Action::Play( track_id ) => {
        if let Some( sequencer ) = self.get_current_device_mut() {
          sequencer.play( track_id );
        }

        scene::SceneEvent::None
      }
      Action::SelectDevice( device_id ) => {
        self.set_current_device( Some( device_id ) );
        scene::SceneEvent::Push( SceneId::ViewDevice )
      }
      Action::Stop => {
        if let Some( sequencer ) = self.get_current_device_mut() {
          sequencer.stop();
        }

        scene::SceneEvent::None
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene Definitions

pub fn build_scenes( state: Arc<RwLock<State>> ) -> ( scene::Foreground<State>, scene::Background<State> ) {
  scene::Builder::new( state, SceneId::ListDevices, Box::new( list::ListScene::new() ) )
    .add_scene( SceneId::ViewDevice, Box::new( device::DeviceScene::new() ) )
    .build()
}