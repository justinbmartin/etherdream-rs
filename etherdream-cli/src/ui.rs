use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use etherdream;
use tokio::sync::RwLock;

use crate::executors;
use crate::scene::{ self, SceneController, SceneEvent };
use crate::sequencer::Sequencer;

mod common;
mod device;
mod list;

#[derive( Debug )]
pub enum Action {
  Back,
  Connect,
  Disconnect,
  Play( executors::TrackId ),
  SelectDevice( IpAddr ),
  Stop
}

#[derive( Clone, Copy, Debug, Eq, Hash, PartialEq )]
pub enum SceneId {
  ListDevices,
  ViewDevice
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

#[derive( Default )]
pub struct State {
  // The currently selected device.
  current_device_id: Option<IpAddr>,
  // A registry of all devices that have been discovered, by IP address.
  device_info_map: HashMap<IpAddr,etherdream::DeviceInfo>,
  // Integer that is bumped each time a device is registered. Used for cache
  // invalidation by scene views.
  devices_version: usize,
  // A map of active sequencers, using the device's IP address as key.
  sequencers: HashMap<IpAddr,Sequencer<executors::TrackInfo>>
}

impl State {
  /// Returns an immutable reference to the `State`s registered devices.
  pub fn devices( &self ) -> &HashMap<IpAddr,etherdream::DeviceInfo> { &self.device_info_map }

  ///
  pub fn devices_version( &self ) -> usize { self.devices_version }

  /// Registers a device from the registry into the `DeviceMap` by `address`.
  pub async fn register_device( &mut self, info: etherdream::DeviceInfo ) {
    self.device_info_map.insert( info.ip(), info );
    self.devices_version = self.devices_version.saturating_add( 1 );
  }
  /// Set the currently selected device id
  pub fn set_current_device( &mut self, device_id: Option<IpAddr> ) {
    self.current_device_id = device_id
  }

  /// Get an immutable reference to the currently selected device
  pub fn get_current_device( &self ) -> Option<&etherdream::DeviceInfo> {
    self.current_device_id.as_ref().and_then(| addr | self.device_info_map.get( addr ) )
  }

  pub fn get_current_sequencer( &self ) -> Option<&Sequencer<executors::TrackInfo>> {
    self.current_device_id.as_ref().and_then(| addr | self.sequencers.get( addr ) )
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
        if let Some( device_info ) = self.get_current_device() {
          match etherdream::connect( *device_info ).await {
            Ok( client ) => {
              let sequencer = Sequencer::new( client, executors::TRACKS.clone() );
              self.sequencers.insert( device_info.ip(), sequencer );
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
          if let Some( sequencer ) = self.sequencers.remove( &device.ip() ) {
            let _ = sequencer.disconnect().await;
          }
        }

        SceneEvent::None
      }
      Action::Play( track_id ) => {
        if let Some( device ) = self.get_current_device() {
          if let Some( sequencer ) = self.sequencers.get_mut( &device.ip() ) {
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
          if let Some( sequencer ) = self.sequencers.get_mut( &device.ip() ) {
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