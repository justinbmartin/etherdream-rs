use std::collections::BTreeMap;
use std::sync::{ Arc, RwLock };

use etherdream::generator;

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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Sequencer

pub struct Sequencer {
  generator: etherdream::Generator,
  state: Arc<RwLock<State>>
}

impl Sequencer {
  pub fn new( client: etherdream::Client ) -> Self {
    let mut tracks: BTreeMap<String, Box<dyn Track>> = BTreeMap::new();

    for ( _, definition ) in tracks::DEFINITIONS.iter() {
      tracks.insert( definition.id.clone(), ( definition.create_fn )() );
    }

    let state = Arc::new( RwLock::new( State{ current: None, tracks } ) );

    let exe = Executor{ state: state.clone() };
    let generator = etherdream::make_generator( client, Box::new( exe ) );
    Self{ generator, state }
  }

  pub fn state( &self ) -> tokio::sync::watch::Ref<'_, etherdream::State> { self.generator.state() }

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

  pub async fn shutdown( self ) -> Result<etherdream::Client,generator::Error> {
     self.generator.into_client().await
  }
}