use std::collections::BTreeMap;
use std::sync::Arc;

use etherdream::generator;
use tokio::sync::RwLock;

use crate::tracks;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

struct State {
  current: Option<&'static str>,
  tracks: BTreeMap<&'static str, Box<dyn Track>>
}

impl generator::Executable for State {
  fn on_start( &mut self, _: generator::OnStartContext ) -> generator::Config {
    generator::Config::new( 1_000 )
  }

  fn execute( &mut self, _ctx: &mut generator::ExecutionContext ) {
    todo!()
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Track

pub trait Track: Send + Sync {
  /// Returns the identifier for this track.
  fn id( &self ) -> &str;

  /// Returns the display name for this track.
  fn name( &self ) -> &str;

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
    let mut tracks: BTreeMap<&'static str, Box<dyn Track>> = BTreeMap::new();
    tracks.insert( "beam", Box::new( tracks::Beam::default() ) );
    tracks.insert( "windmill", Box::new( tracks::Windmill::new() ) );

    let state = Arc::new( RwLock::new( State{ current: None, tracks } ) );

    let generator = etherdream::make_generator( client, Box::new( state.clone() ) );
    Self{ generator, state }
  }

  pub fn shutdown( self ) -> Result<etherdream::Client,generator::Error> {
     self.generator.into_client()
  }
}