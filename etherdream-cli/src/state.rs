use std::collections::{ HashMap, hash_map::Iter };
use std::sync::Arc;

use tokio::sync::{ Mutex, MutexGuard };

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

#[derive( Default )]
pub struct State {
  pub device_id: Arc<Mutex<Option<usize>>>,
  pub device_map: Arc<Mutex<DeviceMap>>
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
        let device_id = self.device_id.lock().await.unwrap();

        if let Some( device ) = self.device_map.lock().await.get_mut( device_id ) {
          let _ = device.connect().await;
          return scene::SceneEvent::None;
        }

        scene::SceneEvent::Pop
      },
      Action::SelectDevice( id ) => {
        *self.device_id.lock().await = Some( id );
        scene::SceneEvent::Switch( scenes::device::ID )
      },
      Action::DeselectDevice => {
        *self.device_id.lock().await = None;
        scene::SceneEvent::Switch( scenes::list::ID )
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device

pub struct Device {
  _id: usize,
  info: etherdream::DeviceInfo,
  client: Option<etherdream::Client>,
  generator: Option<etherdream::Generator>
}

impl Device {
  /// Called from `DeviceMap` to create a new `Device`.
  fn new( id: usize, info: etherdream::DeviceInfo ) -> Self {
    Self{
      client: None,
      _id: id,
      info,
      generator: None
    }
  }

  /// Returns the id of the device
  pub fn _id( &self ) -> usize { self._id }

  /// Returns true if the device is connected.
  pub fn is_connected( &self ) -> bool { self.client.is_some() || self.generator.is_some() }

  //
  pub async fn connect( &mut self ) -> Result<(), etherdream::client::Error> {
    if self.generator.is_some() { return Ok( () ); }

    match etherdream::connect( self.info ).await {
      Ok( client ) => {
        self.client = Some( client );
        Ok( () )
      }
      Err( err ) => Err( err )
    }
  }

  //
  pub async fn disconnect( &mut self ) {
    if let Some( generator ) = self.generator.take() && let Ok( client ) = generator.into_client().await {
      client.disconnect().await
    }
  }

  /// Returns a reference to the active device generator, if one is set.
  pub fn generator( &self ) -> Option<&etherdream::Generator> { self.generator.as_ref() }

  /// Creates and starts a generator from `executable`.
  pub async fn generate( &mut self, executable: Box<dyn etherdream::generator::Executable> ) {
    if let Some( client ) = self.client.take() {
      let mut generator = etherdream::make_generator( client, executable );
      generator.start().await;

      self.generator = Some( generator );
    }
  }

  pub fn info( &self ) -> &etherdream::DeviceInfo {
    &self.info
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device Map

pub struct DeviceMap {
  inner: HashMap<usize,Device>,
  version: usize
}

impl Default for DeviceMap {
  fn default() -> Self {
    Self{
      inner: HashMap::new(),
      version: 0
    }
  }
}

impl DeviceMap{
  /// Returns an immutable reference to a device.
  pub fn get( &self, id: usize ) -> Option<&Device> { self.inner.get( &id ) }
  pub fn get_mut( &mut self, id: usize ) -> Option<&mut Device> { self.inner.get_mut( &id ) }

  /// Inserts a new device into the map, incrementing the map version.
  pub fn insert( &mut self, info: etherdream::DeviceInfo ) {
    let id = self.version;
    self.inner.insert( id, Device::new( id, info ) );
    self.version = self.version.saturating_add( 1 );
  }

  /// Returns an iterator of devices.
  pub fn iter( &self ) -> Iter<'_, usize, Device> {
    self.inner.iter()
  }

  /// Returns the number of devices in the map.
  pub fn len( &self ) -> usize {
    self.inner.len()
  }

  /// Returns the current version of the map.
  pub fn version( &self ) -> usize {
    self.version
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - Read-only Device Map

pub struct ReadOnlyDeviceMap {
  inner: Arc<Mutex<DeviceMap>>
}

impl ReadOnlyDeviceMap {
  pub fn new( device_map: Arc<Mutex<DeviceMap>> ) -> Self {
    Self{ inner: device_map }
  }

  pub fn read( &'_ self ) -> MutexGuard<'_, DeviceMap> {
    self.inner.blocking_lock()
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device Guard

pub struct DeviceGuard<'a> {
  guard: MutexGuard<'a, DeviceMap>,
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

#[derive( Clone )]
pub struct ScopedDevice {
  device_map: Arc<Mutex<DeviceMap>>,
  device_id: Arc<Mutex<Option<usize>>>
}

impl ScopedDevice {
  pub fn new( device_id: Arc<Mutex<Option<usize>>>, device_map: Arc<Mutex<DeviceMap>> ) -> Self {
    Self{ device_id, device_map }
  }

  pub fn get( &'_ self ) -> Option<DeviceGuard<'_>> {
    let device_id =
      match self.device_id.try_lock() {
        Ok( guard ) => *guard,
        Err( _ ) => None
      };

    match device_id {
      Some( device_id ) => {
        let device_map = self.device_map.blocking_lock();
        Some( DeviceGuard{ guard: device_map, device_id })
      }
      None => None
    }
  }
}