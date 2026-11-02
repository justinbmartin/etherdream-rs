use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{ Arc, RwLock };

use etherdream::generator;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Track

pub trait Track: Send + Sync {
  /// Called by the sequencer to generate point-data for this track.
  fn execute( &mut self, ctx: &mut generator::ExecutionContext );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

struct State<T> {
  current: Option<T>,
}

pub struct Sequencer<T> {
  generator: etherdream::Generator,
  state: Arc<RwLock<State<T>>>
}

impl<T> Sequencer<T>
  where T: 'static + Clone + Copy + Eq + Hash + Send + Sync
{
  pub fn new( client: etherdream::Client, tracks: HashMap<T, Box<dyn Track>> ) -> Self {
    let state = Arc::new( RwLock::new( State::<T>{ current: None } ) );
    let executor = Box::new( Executor::new( state.clone(), tracks ) );
    let generator = etherdream::make_generator( client, executor );

    Self{ generator, state }
  }

  pub async fn disconnect( self ) -> Result<etherdream::Client,generator::Error> {
    self.generator.into_client().await
  }

  pub fn current( &self ) -> Option<T> {
    self.state.read().ok().map(| s | s.current )?
  }

  pub fn play( &mut self, track_id: Option<T> ) {
    if let Ok( mut state ) = self.state.write() {
      state.current = track_id;
    }
  }

  pub fn is_playing( &self ) -> bool {
    if let Ok( state ) = self.state.read() {
      state.current.is_some()
    } else {
      false
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Executor

pub struct Executor<T: Eq + Hash> {
  state: Arc<RwLock<State<T>>>,
  tracks: HashMap<T, Box<dyn Track>>
}

impl<T: Eq + Hash> Executor<T> {
  pub fn new( state: Arc<RwLock<State<T>>>, tracks: HashMap<T, Box<dyn Track>> ) -> Self {
    Self{ state, tracks }
  }
}

impl<T> generator::Executable for Executor<T>
  where T: 'static + Eq + Hash + Send + Sync
{
  fn on_start( &mut self, _: generator::OnStartContext ) -> generator::Config {
    generator::Config::new( 1_000 )
  }

  fn execute( &mut self, ctx: &mut generator::ExecutionContext ) {
    if let Ok( state ) = self.state.read() && let Some( current ) = state.current.as_ref() {
      if let Some( track ) = self.tracks.get_mut( &current ) {
        track.execute( ctx )
      }
    }
  }
}