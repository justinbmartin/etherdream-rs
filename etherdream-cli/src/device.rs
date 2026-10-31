use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{ Arc, RwLock };

use etherdream::{ generator, protocol };

use crate::tracks;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

struct State {
  current: Option<&'static str>,
  tracks: BTreeMap<String, Box<dyn Track>>
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Executor

struct Executor {
  state: Arc<RwLock<State>>
}

impl generator::Executable for Executor {
  fn on_start( &mut self, _: generator::OnStartContext ) -> generator::Config {
    generator::Config::new( 1_000 )
  }

  fn execute( &mut self, ctx: &mut generator::ExecutionContext ) {
    if let Ok( mut state ) = self.state.write() {
      if let Some( current ) = state.current && let Some( track ) = state.tracks.get_mut( current ) {
        track.execute( ctx )
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Track

pub trait Track: Send + Sync {
  /// Called by the sequencer to generate point-data for this track.
  fn execute( &mut self, ctx: &mut generator::ExecutionContext );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device

pub struct Device {
  info: etherdream::DeviceInfo,
  generator: Option<etherdream::Generator>,
  state: Arc<RwLock<State>>
}

impl Device {
  pub fn new( info: etherdream::DeviceInfo ) -> Self {
    // TODO: not sure this required...
    let mut tracks: BTreeMap<String, Box<dyn Track>> = BTreeMap::new();

    for definition in tracks::DEFINITIONS.iter() {
      tracks.insert( definition.id.clone(), ( definition.create_fn )() );
    }

    let state = Arc::new( RwLock::new( State{ current: None, tracks } ) );
    Self{ generator: None, info, state }
  }

  pub fn info( &self ) -> &etherdream::DeviceInfo { &self.info }

  pub async fn connect( &mut self ) -> Result<(), etherdream::client::Error> {
    if self.generator.is_some() { return Ok( () ); }

    match etherdream::connect( self.info ).await {
      Ok( client ) => {
        let exe = Executor{ state: self.state.clone() };
        self.generator = Some( etherdream::make_generator( client, Box::new( exe ) ) );
        Ok( () )
      }
      Err( err ) => Err( err )
    }
  }

  pub async fn disconnect( &mut self ) {
    if let Some( generator ) = self.generator.take() && let Ok( client ) = generator.into_client().await {
      client.disconnect().await
    }
  }

  pub fn address( &self ) -> SocketAddr { SocketAddr::new( self.info.ip(), protocol::CLIENT_PORT ) }

  pub fn is_connected( &self ) -> bool { self.generator.is_some() }

  pub fn state( &self ) -> Option<tokio::sync::watch::Ref<'_, etherdream::State>> {
    self.generator.as_ref().map(| g | g.state() )
  }

  pub fn play( &self, track_id: &'static str ) {
    if let Ok( mut state ) = self.state.write() {
      state.current = Some( track_id );
    }
  }

  ///
  pub fn is_playing( &self ) -> bool {
    if let Ok( state ) = self.state.read() {
      state.current.is_some()
    } else {
      false
    }
  }

  pub fn stop( &self ) {
    if let Ok( mut state ) = self.state.write() {
      state.current = None;
    }
  }
}