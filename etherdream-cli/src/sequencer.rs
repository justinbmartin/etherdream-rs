use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{ Arc, RwLock };

use etherdream::generator;
use tokio::sync::watch;

type CreateTrackExecutorFn = fn() -> Box<dyn TrackExecutor>;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Track

/// The trait that must be implemented by all track executors. Responsible for
/// generating point-data.
pub trait TrackExecutor: Send + Sync {
  /// Called by the sequencer to generate point-data for the track.
  fn execute( &mut self, ctx: &mut generator::ExecutionContext );
}

/// A trait used to describe required and any implementor-defined properties
/// for a track.
pub trait Track: 'static + Clone {
  type Id: Clone + Copy + Eq + Hash + Send + Sync;

  /// Returns the ID for this track. The ID must be unique across all tracks in
  /// the sequencer.
  fn id( &self ) -> &Self::Id;
}

/// Describes a track's properties and the function to create it.
#[derive( Clone, Copy )]
pub struct TrackDefinition<T: Track> {
  create_fn: CreateTrackExecutorFn,
  track: T
}

impl<T: Track> TrackDefinition<T> {
  /// Creates a new track definition.
  pub fn new( track: T, create_fn: CreateTrackExecutorFn ) -> Self {
    Self{ create_fn, track }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Sequencer

/// Consumes a client and provides for sequencer capabilities: tracks, the
/// ability to switch between tracks, start/stop, etc.
pub struct Sequencer<T: Track> {
  current: Arc<RwLock<Option<T::Id>>>,
  generator: etherdream::Generator,
  tracks: Vec<T>
}

impl<T: Track> Sequencer<T> {
  pub fn new( client: etherdream::Client, track_definitions: Vec<TrackDefinition<T>> ) -> Self {
    let current = Arc::new( RwLock::new( None ) );

    let create_track_executor_fn_map = track_definitions.iter().map(| td | ( *td.track.id(), td.create_fn )).collect();
    let tracks = track_definitions.into_iter().map(| td | td.track ).collect();

    let executor = Box::new( Executor::<T>::new( current.clone(), create_track_executor_fn_map ) );
    let generator = etherdream::make_generator( client, executor );
    Self{ current, generator, tracks }
  }

  /// Consumes the sequencer and returns the underlying `etherdream::Client`.
  pub async fn into_client( self ) -> Result<etherdream::Client,generator::Error> {
    self.generator.into_client().await
  }

  /// Returns all tracks registered with the sequencer.
  pub fn tracks( &self ) -> &Vec<T> { &self.tracks }

  /// Plays a track by `track_id`. Does nothing is the track is already
  /// playing.
  pub fn play( &mut self, track_id: T::Id ) {
    if let Ok( mut current ) = self.current.write() {
      *current = Some( track_id );
    }
  }

  /// Returns true if the sequencer is currently playing.
  pub fn is_playing( &self ) -> bool {
    self.current.read().is_ok_and(| c | c.is_some() )
  }

  /// Stops the sequencer.
  pub fn stop( &mut self ) {
    if let Ok( mut current ) = self.current.write() {
      *current = None;
    }
  }

  /// Returns a reference to the internal device state associated with the
  /// `etherdream::Client`.
  pub fn state( &'_ self ) -> watch::Ref<'_, etherdream::State> {
    self.generator.state()
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Executor

// The Etherdream generator that drives the `Sequencer` asynchronously.
struct Executor<T: Track> {
  create_track_executor_fn_map: HashMap<T::Id, CreateTrackExecutorFn>,
  current_track: Arc<RwLock<Option<T::Id>>>,
  executor: Option<Box<dyn TrackExecutor>>,
  local_current: Option<T::Id>
}

impl<T: Track> Executor<T> {
  pub fn new(
    current_track: Arc<RwLock<Option<T::Id>>>,
    create_track_executor_fn_map: HashMap<T::Id,CreateTrackExecutorFn>
  ) -> Self {
    Self{ create_track_executor_fn_map, current_track, executor: None, local_current: None }
  }
}

impl<T: Track> generator::Executable for Executor<T> {
  fn on_start( &mut self, _: generator::OnStartContext ) -> generator::Config {
    generator::Config::new( 1_000 )
  }

  fn execute( &mut self, ctx: &mut generator::ExecutionContext ) {
    // Observe if we need to change tracks.
    if let Ok( next ) = self.current_track.read() {
      match ( &self.local_current, *next ) {
        ( current, Some( next ) ) => {
          if current.is_none_or(| c | c != next ) && let Some( create_fn ) = self.create_track_executor_fn_map.get( &next ) {
            self.executor = Some( create_fn() );
            self.local_current = Some( next );
          }
        }
        ( Some( _ ), None ) => {
          self.executor = None;
          self.local_current = None;
        }
        ( None, None ) => { /* no-op */ }
      }
    }

    if let Some( executor ) = self.executor.as_mut() {
      executor.execute( ctx );
    }
  }
}