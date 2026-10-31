use std::collections::BTreeMap;
use std::sync::{ Arc, RwLock };

use etherdream::generator;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Track

pub trait Track: Send + Sync {
  /// Called by the sequencer to generate point-data for this track.
  fn execute( &mut self, ctx: &mut generator::ExecutionContext );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

#[derive( Default )]
pub struct State {
  current: Option<&'static str>
}

impl State {
  pub fn current( &self ) -> &Option<&'static str> { &self.current }
  pub fn set_current( &mut self, track_id: Option<&'static str> ) { self.current = track_id; }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Executor

pub struct Sequencer {
  state: Arc<RwLock<State>>,
  tracks: BTreeMap<String, Box<dyn Track>>
}

impl Sequencer {
  pub fn new( state: Arc<RwLock<State>>, tracks: BTreeMap<String, Box<dyn Track>> ) -> Self {
    Self{ state, tracks }
  }
}

impl generator::Executable for Sequencer {
  fn on_start( &mut self, _: generator::OnStartContext ) -> generator::Config {
    generator::Config::new( 1_000 )
  }

  fn execute( &mut self, ctx: &mut generator::ExecutionContext ) {
    let current = if let Ok( state ) = self.state.read() { state.current.unwrap() } else { return };

    if let Some( track ) = self.tracks.get_mut( current ) {
      track.execute( ctx )
    }
  }
}