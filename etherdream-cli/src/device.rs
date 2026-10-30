use std::collections::HashMap;
use std::collections::hash_map::Iter;
use std::net::SocketAddr;

use etherdream::protocol;

use crate::sequencer;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device

pub struct Device {
  info: etherdream::DeviceInfo,
  sequencer: Option<sequencer::Sequencer>,
}

impl Device {
  /// Called from `DeviceMap` to create a new `Device`.
  fn new( info: etherdream::DeviceInfo ) -> Self {
    Self{ info, sequencer: None }
  }

  /// Returns the address that the client will communicate on.
  pub fn address( &self ) -> SocketAddr { SocketAddr::new( self.info.ip(), protocol::CLIENT_PORT ) }

  /// Returns true if the device is connected.
  pub fn is_connected( &self ) -> bool { self.sequencer.is_some() }

  /// ...
  pub fn state( &self, mut state: &mut etherdream::State ) {
    if let Some( client ) = self.client.as_ref() {
      client.clone_state_into( &mut state );
    } else if let Some( generator ) = self.generator() {
      generator.clone_state_into( &mut state );
    } else {
      // uh-oh...
    }
  }

  //
  pub async fn connect( &mut self ) -> Result<(), etherdream::client::Error> {
    if self.sequencer.is_some() { return Ok( () ); }

    match etherdream::connect( self.info ).await {
      Ok( client ) => {
        self.sequencer = Some( sequencer::Sequencer::new( client ) );
        Ok( () )
      }
      Err( err ) => Err( err )
    }
  }

  //
  pub async fn disconnect( &mut self ) {
    if let Some( sequencer ) = self.sequencer.take() && let Ok( client ) = sequencer.shutdown() {
      client.disconnect().await
    }
  }

  /// Returns a reference to the active device generator, if one is set.
  pub fn sequencer( &self ) -> Option<&sequencer::Sequencer> { self.sequencer.as_ref() }
  pub fn sequencer_mut( &mut self ) -> Option<&mut sequencer::Sequencer> { self.sequencer.as_mut() }

  ///
  pub fn info( &self ) -> &etherdream::DeviceInfo { &self.info }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device Map

#[derive( Default )]
pub struct VersionedDeviceMap {
  inner: HashMap<SocketAddr,Device>,
  version: usize
}

impl VersionedDeviceMap {
  /// Returns an immutable reference to a device.
  pub fn get( &self, address: &SocketAddr ) -> Option<&Device> { self.inner.get( address ) }
  pub fn get_mut( &mut self, address: &SocketAddr ) -> Option<&mut Device> { self.inner.get_mut( address ) }

  /// Inserts a new device into the map, incrementing the map version.
  pub fn insert( &mut self, address: SocketAddr, info: etherdream::DeviceInfo ) -> Option<Device> {
    let r = self.inner.insert( address, Device::new( info ) );
    self.version = self.version.saturating_add( 1 );
    r
  }

  /// Returns an iterator of devices.
  pub fn iter( &self ) -> Iter<'_, SocketAddr, Device> {
    self.inner.iter()
  }

  /// Returns the current version of the map.
  pub fn version( &self ) -> usize {
    self.version
  }
}