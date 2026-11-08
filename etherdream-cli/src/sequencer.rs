use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{ Arc, RwLock };

use etherdream::generator;
use tokio::sync::watch;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Track

/// The trait that must be implemented by all track executors. Responsible for
/// generating point-data.
pub trait TrackExecutor: Send + Sync {
  /// Called by the sequencer to generate point-data for the track.
  fn execute( &mut self, ctx: &mut generator::ExecutionContext );
}

/// A trait used to describe the required and any implementor-defined
/// properties for a track.
pub trait Track: Clone {
  type TrackId: 'static + Clone + Copy + Eq + Hash + Send + Sync;

  /// Returns the ID for this track. The ID must be unique across all tracks in
  /// the sequencer.
  fn id( &self ) -> &Self::TrackId;
}

/// Describes a tracks properties and a function to create it.
#[derive( Clone, Copy )]
pub struct TrackDefinition<T: Track> {
  create_fn: fn() -> Box<dyn TrackExecutor>,
  track: T
}

impl<T: Track> TrackDefinition<T> {
  /// Creates a new track definition.
  pub fn new( track: T, create_fn: fn() -> Box<dyn TrackExecutor> ) -> Self {
    Self{ create_fn, track }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

pub struct Sequencer<T: Track> {
  current: Arc<RwLock<Option<T::TrackId>>>,
  generator: etherdream::Generator,
  tracks: Vec<T>
}

impl<T: Track> Sequencer<T> {
  pub fn new( client: etherdream::Client, track_definitions: Vec<TrackDefinition<T>> ) -> Self {
    let current = Arc::new( RwLock::new( None::<T::TrackId> ) );

    let track_create_fn_map = track_definitions.iter().map(| td | ( *td.track.id(), td.create_fn )).collect();
    let tracks = track_definitions.into_iter().map(| td | td.track ).collect();

    let executor = Box::new( Executor::new( current.clone(), track_create_fn_map ) );
    let generator = etherdream::make_generator( client, executor );
    Self{ generator, current, tracks }
  }

  pub async fn disconnect( self ) -> Result<etherdream::Client,generator::Error> {
    self.generator.into_client().await
  }

  /// Returns a list of all track id's recognized by the sequencer.
  pub fn tracks( &self ) -> &Vec<T> { &self.tracks }

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
  track: Option<Box<dyn TrackExecutor>>,
  track_create_fn_map: HashMap<T, fn() -> Box<dyn TrackExecutor>>
}

impl<T: Clone + Copy + Eq + Hash> Executor<T> {
  pub fn new( current: Arc<RwLock<Option<T>>>, track_create_fn_map: HashMap<T, fn() -> Box<dyn TrackExecutor>> ) -> Self {
    Self{ local_current: None, current, track: None, track_create_fn_map }
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
          if current.is_none_or(| c | *c != next ) && let Some( create_fn ) = self.track_create_fn_map.get( &next ) {
            self.track = Some( create_fn() );
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