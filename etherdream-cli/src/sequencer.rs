use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::sync::{ Arc, RwLock };

use etherdream::generator;
use tokio::sync::watch;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Track

pub trait TrackExecutor: Send + Sync {
  /// Called by the sequencer to generate point-data for this track.
  fn execute( &mut self, ctx: &mut generator::ExecutionContext );
}

// `T` here is what they track id type is. Could be a &str, String, enum, etc...
#[derive( Clone )]
pub struct Track<T: Clone + Copy> {
  pub create_executor_fn: fn() -> Box<dyn TrackExecutor>,
  pub definition: T
}

impl<T: Clone + Copy> Track<T> {
  pub fn new( definition: T, create_executor_fn: fn() -> Box<dyn TrackExecutor> ) -> Self {
    Self{ create_executor_fn, definition }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

// TODO: I don't think this is needed?
struct State<T: Clone + Copy> {
  current: Option<T>
}

pub struct Sequencer<T: Clone + Copy + Send + Sync> {
  generator: etherdream::Generator,
  state: Arc<RwLock<State<T>>>,
  tracks: Vec<Track<T>>
}

impl<T> Sequencer<T>
  where T: 'static + Clone + Copy + Display + Eq + Hash + Send + Sync
{
  pub fn new( client: etherdream::Client, tracks: Vec<Track<T>> ) -> Self {
    let state = Arc::new( RwLock::new( State::<T>{ current: None } ) );
    let executor = Box::new( Executor::new( state.clone(), tracks.clone() ) );
    let generator = etherdream::make_generator( client, executor );

    Self{ generator, state, tracks }
  }

  pub async fn disconnect( self ) -> Result<etherdream::Client,generator::Error> {
    self.generator.into_client().await
  }

  pub fn tracks( &self ) -> &Vec<Track<T>> { &self.tracks }

  pub fn play( &mut self, track_id: T ) {
    if let Ok( mut state ) = self.state.write() {
      state.current = Some( track_id );
    }
  }

  pub fn is_playing( &self ) -> bool {
    if let Ok( state ) = self.state.read() {
      state.current.is_some()
    } else {
      false
    }
  }

  pub fn stop( &mut self ) {
    if let Ok( mut state ) = self.state.write() {
      state.current = None;
    }
  }

  pub fn state( &'_ self ) -> watch::Ref<'_, etherdream::State> {
    self.generator.state()
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Executor

struct Executor<T: Clone + Copy> {
  current: Option<T>,
  state: Arc<RwLock<State<T>>>,
  track: Option<Box<dyn TrackExecutor>>,
  track_definitions: HashMap<T,Track<T>>
}

impl<T: Clone + Copy + Eq + Hash> Executor<T> {
  pub fn new( state: Arc<RwLock<State<T>>>, tracks: Vec<Track<T>> ) -> Self {
    let tracks: HashMap<T,Track<T>> = tracks.into_iter().map(| t | ( t.definition.id, t )).collect();
    Self{ current: None, state, track: None, track_definitions: tracks }
  }
}

impl<T> generator::Executable for Executor<T>
  where T: Clone + Copy + Eq + Hash + Send + Sync
{
  fn on_start( &mut self, _: generator::OnStartContext ) -> generator::Config {
    generator::Config::new( 1_000 )
  }

  fn execute( &mut self, ctx: &mut generator::ExecutionContext ) {
    if let Ok( state ) = self.state.read() {
      match ( self.current.as_ref(), state.current ) {
        ( current, Some( next ) ) => {
          if current.is_none_or(| c | *c != next ) && let Some( track_definition ) = self.track_definitions.get( &next ) {
            self.track = Some( ( track_definition.create_executor_fn )() );
            self.current = Some( next );
          }
        }
        ( Some( _ ), None ) => {
          self.track = None;
          self.current = None;
        }
        ( None, None ) => { /* no-op */ }
      }
    }

    if let Some( track ) = self.track.as_mut() {
      track.execute( ctx );
    }
  }
}