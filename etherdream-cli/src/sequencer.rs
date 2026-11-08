use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{ Arc, RwLock };

use etherdream::generator;
use tokio::sync::watch;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Track

/// The trait that must be implemented by all track executors in the sequencer.
pub trait Track: Send + Sync {
  /// Called by the sequencer to generate point-data for this track.
  fn execute( &mut self, ctx: &mut generator::ExecutionContext );
}

/// Defines how to create a track and its varying properties.
#[derive( Clone )]
pub struct TrackDefinition<T: TrackInfo> {
  create_executor_fn: fn() -> Box<dyn Track>,
  info: T
}

impl<T: TrackInfo> TrackDefinition<T> {
  pub fn new( info: T, create_executor_fn: fn() -> Box<dyn Track> ) -> Self {
    Self{ create_executor_fn, info }
  }
}

pub trait TrackInfo: Clone {
  type TrackId: 'static + Clone + Copy + Eq + Hash + Send + Sync;

  fn id( &self ) -> &Self::TrackId;
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

pub struct Sequencer<T: TrackInfo> {
  current: Arc<RwLock<Option<T::TrackId>>>,
  generator: etherdream::Generator,
  track_ids: Vec<T::TrackId>
}

impl<T: TrackInfo> Sequencer<T> {
  pub fn new( client: etherdream::Client, track_definitions: Vec<TrackDefinition<T::TrackId>> ) -> Self {
    let current = Arc::new( RwLock::new( None ) );
    let executor = Box::new( Executor::new( current.clone(), track_definitions.clone() ) );
    let generator = etherdream::make_generator( client, executor );

    let track_ids = track_definitions.iter().map(| td | td.id ).collect();
    Self{ generator, current, track_ids }
  }

  pub async fn disconnect( self ) -> Result<etherdream::Client,generator::Error> {
    self.generator.into_client().await
  }

  /// Returns a list of all track id's recognized by the sequencer.
  pub fn track_ids( &self ) -> &Vec<T::TrackId> { &self.track_ids }

  pub fn play( &mut self, track_id: T::TrackId ) {
    if let Ok( mut current ) = self.current.write() {
      *current = Some( track_id );
    }
  }

  pub fn is_playing( &self ) -> bool {
    if let Ok( current ) = self.current.read() {
      current.is_some()
    } else {
      false
    }
  }

  pub fn stop( &mut self ) {
    if let Ok( mut current ) = self.current.write() {
      *current = None;
    }
  }

  pub fn state( &'_ self ) -> watch::Ref<'_, etherdream::State> {
    self.generator.state()
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Executor

struct Executor<T: Clone + Copy> {
  local_current: Option<T>,
  current: Arc<RwLock<Option<T>>>,
  track: Option<Box<dyn Track>>,
  track_definitions: HashMap<T,TrackDefinition<T>>
}

impl<T: Clone + Copy + Eq + Hash> Executor<T> {
  pub fn new( current: Arc<RwLock<Option<T>>>, track_definitions: Vec<TrackDefinition<T>> ) -> Self {
    let track_definitions = track_definitions.into_iter().map(| td | ( td.id, td )).collect();
    Self{ local_current: None, current, track: None, track_definitions }
  }
}

impl<T> generator::Executable for Executor<T>
  where T: Clone + Copy + Eq + Hash + Send + Sync
{
  fn on_start( &mut self, _: generator::OnStartContext ) -> generator::Config {
    generator::Config::new( 1_000 )
  }

  fn execute( &mut self, ctx: &mut generator::ExecutionContext ) {
    if let Ok( current ) = self.current.read() {
      match ( self.local_current.as_ref(), *current ) {
        ( current, Some( next ) ) => {
          if current.is_none_or(| c | *c != next ) && let Some( track_definition ) = self.track_definitions.get( &next ) {
            self.track = Some( ( track_definition.create_executor_fn )() );
            self.local_current = Some( next );
          }
        }
        ( Some( _ ), None ) => {
          self.track = None;
          self.local_current = None;
        }
        ( None, None ) => { /* no-op */ }
      }
    }

    if let Some( track ) = self.track.as_mut() {
      track.execute( ctx );
    }
  }
}