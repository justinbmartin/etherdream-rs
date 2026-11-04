use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use etherdream::{ self, protocol };
use tokio::sync::RwLock;

use crate::device::Device;
use crate::executors;
use crate::scene::{ self, SceneController, SceneEvent };
use crate::sequencer::{ self, Sequencer };

mod common;
mod device;
mod list;

#[derive( Debug )]
pub enum Action {
  Back,
  Connect,
  Disconnect,
  Play( executors::TrackId ),
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
  devices: HashMap<SocketAddr,Device>,
  //
  devices_version: usize,
  //
  sequencers: HashMap<SocketAddr,Sequencer<executors::TrackId>>
}

impl State {
  /// Creates a new `State` instance.
  pub fn new() -> Self {
    Self{
      current_device_id: None,
      devices: HashMap::default(),
      devices_version: 0,
      sequencers: HashMap::default()
    }
  }

  /// Returns an immutable reference to the `State`s registered devices.
  pub fn devices( &self ) -> &HashMap<SocketAddr,Device> { &self.devices }

  /// Returns an immutable reference to the `State`s registered devices.
  pub fn sequencers( &self ) -> &HashMap<SocketAddr,Sequencer<executors::TrackId>> { &self.sequencers }

  ///
  pub fn devices_version( &self ) -> usize { self.devices_version }

  /// Registers a device from the registry into the `DeviceMap` by `address`.
  pub async fn register_device( &mut self, info: etherdream::DeviceInfo ) {
    self.devices.insert( SocketAddr::new( info.ip(), protocol::CLIENT_PORT ), Device::new( info ) );
    self.devices_version = self.devices_version.saturating_add( 1 );
  }
  /// Set the currently selected device id
  pub fn set_current_device( &mut self, device_id: Option<SocketAddr> ) {
    self.current_device_id = device_id
  }
  

  /// Get an immutable reference to the currently selected device
  pub fn get_current_device( &self ) -> Option<&Device> {
    self.current_device_id.and_then(|d| self.devices.get( &d ) )
  }
}

impl SceneController for State {
  type Action = Action;
  type SceneId = SceneId;

  async fn invoke( &mut self, action: Action ) -> SceneEvent<Self::SceneId> {
    match action {
      Action::Back => {
        SceneEvent::Pop
      }
      Action::Connect => {
        if let Some( device ) = self.get_current_device() {
          match etherdream::connect( *device.info() ).await {
            Ok( client ) => {
              let mut tracks: HashMap<executors::TrackId, Box<dyn sequencer::Track>> = HashMap::new();

              for definition in executors::DEFINITIONS.iter() {
                tracks.insert( definition.id, ( definition.create_fn )() );
              }

              let sequencer = Sequencer::new( client, tracks );
              self.sequencers.insert( device.address(), sequencer );
            }
            Err( _err ) => {
              //Err( err )
            }
          }
        }

        SceneEvent::None
      }
      Action::Disconnect => {
        if let Some( device ) = self.get_current_device() {
          if let Some( sequencer ) = self.sequencers.remove( &device.address() ) {
            let _ = sequencer.disconnect().await;
          }
        }

        SceneEvent::None
      }
      Action::Play( track_id ) => {
        if let Some( device ) = self.get_current_device() {
          if let Some( sequencer ) = self.sequencers.get_mut( &device.address() ) {
            sequencer.play( track_id );
          }
        }

        SceneEvent::None
      }
      Action::SelectDevice( device_id ) => {
        self.set_current_device( Some( device_id ) );
        SceneEvent::Push( SceneId::ViewDevice )
      }
      Action::Stop => {
        if let Some( device ) = self.get_current_device() {
          if let Some( sequencer ) = self.sequencers.get_mut( &device.address() ) {
            sequencer.stop();
          }
        }

        SceneEvent::None
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