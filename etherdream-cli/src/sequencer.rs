use std::hash::Hash;
use std::sync::{ Arc, RwLock };

use etherdream::generator;
use tokio::sync::watch;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Track

pub trait TrackList: Clone + Copy + Send + Sync {
  /// Called by the sequencer to generate point-data for this track.
  fn new( &self ) -> Box<dyn Track>;
}

pub trait Track: Send + Sync {
  /// Called by the sequencer to generate point-data for this track.
  fn execute( &mut self, ctx: &mut generator::ExecutionContext );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

// TODO: I don't think this is needed?
struct State<T: TrackList> {
  current: Option<T>,
}

pub struct Sequencer<T: TrackList> {
  generator: etherdream::Generator,
  state: Arc<RwLock<State<T>>>
}

impl<T: 'static + Eq + Hash + TrackList> Sequencer<T> {
  pub fn new( client: etherdream::Client ) -> Self {
    let state = Arc::new( RwLock::new( State::<T>{ current: None } ) );
    let executor = Box::new( Executor::new( state.clone() ) );
    let generator = etherdream::make_generator( client, executor );

    Self{ generator, state }
  }

  pub async fn disconnect( self ) -> Result<etherdream::Client,generator::Error> {
    self.generator.into_client().await
  }

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

struct Executor<T: TrackList> {
  current: Option<T>,
  state: Arc<RwLock<State<T>>>,
  track: Option<Box<dyn Track>>
}

impl<T: TrackList> Executor<T> {
  pub fn new( state: Arc<RwLock<State<T>>> ) -> Self {
    Self{ current: None, state, track: None }
  }
}

impl<T> generator::Executable for Executor<T>
  where T: 'static + Eq + Hash + Send + Sync + TrackList
{
  fn on_start( &mut self, _: generator::OnStartContext ) -> generator::Config {
    generator::Config::new( 1_000 )
  }

  fn execute( &mut self, ctx: &mut generator::ExecutionContext ) {
    let next = self.state.read().map(| s | s.current ).ok().flatten();

    match ( self.current.as_ref(), next ) {
      ( Some( current ), Some( next ) ) => {
        if *current != next {
          self.track = Some( next.new() );
          self.current = Some( next );
        }
      }
      ( Some( _ ), None ) => {
        self.track = None;
        self.current = None;
      }
      ( None, Some( next ) ) => {
        self.track = Some( next.new() );
        self.current = Some( next );
      }
      ( None, None ) => { /* no-op */ }
    }

    if let Some( track ) = self.track.as_mut() {
      track.execute( ctx );
    }
  }
}