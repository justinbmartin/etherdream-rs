use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{ self, EventStream, KeyCode, KeyEvent, KeyEventKind };
use futures::{ FutureExt, StreamExt };
use ratatui::{ buffer::Buffer, layout::Rect };
use tokio::{ sync::{ mpsc, RwLock, RwLockReadGuard }, task::JoinSet, time };
use tokio_util::sync::CancellationToken;

const DEFAULT_FPS: f32 = 30.0;

#[derive( Debug )]
pub enum SceneEvent {
  /// No action.
  None,
  /// Pops the current scene from the stack.
  Pop,
  /// Pushes a new scene on to the stack. Useful for modals.
  Push( &'static str ),
  /// Clears the stack, calling `Scene::on_exit` for each scene. Pushes into
  /// the new scene.
  Switch( &'static str )
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Actionable

pub trait Actionable {
  /// Actions that the user-defined scene Backend supports.
  type Action: 'static + Send + fmt::Debug;

  /// Sends an implementation-defined `Action` from the `Foreground<T>` to the
  /// asynchronous `Background<T>` handler. Must return a `SceneEvent`.
  fn invoke( &mut self, action: Self::Action ) -> impl Future<Output=SceneEvent> + Send;
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene

pub trait Scene<T: Actionable + Send + 'static> {
  /// Called once, before any update or draw, when the scene is entered.
  fn on_enter( &mut self, _ctx: &Context<T> ) { }

  /// Called once, after the last update and draw, when the scene is exited.
  fn on_exit( &mut self, _ctx: &Context<T> ) { }

  /// Called each time a key is pressed if the scene is active. Return true if
  /// this call handled the key event. Return false to bubble the key event up
  /// the scene stack.
  fn on_key_down( &mut self, _key: KeyEvent, _ctx: &mut Context<T> ) -> bool { false }

  /// Called when the scene is requested to update.
  fn on_update( &mut self, _ctx: &mut Context<T> ) { }

  /// Called when the scene is requested to draw a frame. Receives the `area`
  /// it is to render into, and the writable `buf`.
  fn on_draw( &mut self, area: Rect, buf: &mut Buffer, state: &Context<T> );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Builder

pub enum BuilderError {
  SceneRequired
}

pub struct Builder<T: Actionable + Send + 'static> {
  current: Option<&'static str>,
  scenes: HashMap<&'static str, Box<dyn Scene<T>>>,
  state: Arc<RwLock<T>>
}

impl<'a,T: Actionable + Send + 'static> Builder<T> {
  pub fn new( state: Arc<RwLock<T>> ) -> Self {
    Self{
      current: None,
      scenes: HashMap::new(),
      state
    }
  }

  /// Adds a scene to the builder definition.
  pub fn add_scene( &mut self, id: &'static str, scene: Box<dyn Scene<T>> ) -> bool {
    self.scenes.insert( id, scene );
    if self.current.is_none() { self.current = Some( id ) }
    true
  }

  /// Builds the scene definitions, returning back a foreground and background
  /// handler. TODO: elaborate...
  pub fn build( self ) -> Result<( Foreground<T>, Background<T> ),BuilderError> {
    if let Some( current ) = self.current {
      let ( action_tx, action_rx ) = mpsc::channel::<T::Action>( 16 );
      let ( events_tx, events_rx ) = mpsc::channel::<Event>( 1024 );

      Ok( (
        Foreground{
          ctx: Context{ action_tx, state: self.state.clone() },
          events_rx,
          scenes: self.scenes,
          stack: vec![ current ]
        },
        Background{
          action_rx,
          actionable: self.state,
          events_tx
        }
      ) )
    } else {
      Err( BuilderError::SceneRequired )
    }
  }
}

impl fmt::Display for BuilderError {
  fn fmt( &self, f: &mut fmt::Formatter<'_> ) -> fmt::Result {
    match self {
      BuilderError::SceneRequired => write!( f, "At least one scene is required." )
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - -  Foreground UI Handler

pub struct Foreground<T: Actionable + Send + 'static> {
  ctx: Context<T>,
  events_rx: mpsc::Receiver<Event>,
  scenes: HashMap<&'static str, Box<dyn Scene<T>>>,
  stack: Vec<&'static str>
}

impl<T: Actionable + Send + 'static> Foreground<T> {
  pub fn run( mut self ) {
    let mut terminal = ratatui::init();

    while let Some( event ) = self.events_rx.blocking_recv() {
      match event {
        Event::Key( key ) => {
          if let Some( scene ) = self.stack.last().and_then( |current| self.scenes.get_mut( current ) ) {
            if ! scene.on_key_down( key, &mut self.ctx ) {
              match key.code {
                KeyCode::Char( 'q' ) | KeyCode::Esc => { break; },
                _ => { }
              }
            }
          }
        },
        Event::Tick( _time ) => {
          if let Some( scene ) = self.stack.last().and_then( |current| self.scenes.get_mut( current ) ) {
            let _ = scene.on_update( &mut self.ctx );
            let _ = terminal.draw(| frame |{ scene.on_draw( frame.area(), frame.buffer_mut(), &self.ctx ); });
          }
        }
        Event::Scene( event ) => {
          self.on_event( event )
        }
      }
    }
  }

  fn on_event( &mut self, event: SceneEvent ) {
    match event {
      SceneEvent::Push( next_scene_id ) => {
        if let Some( current_id ) = self.stack.last() {
          if let [Some( current_scene ), Some( next_scene )] = self.scenes.get_disjoint_mut([ *current_id, next_scene_id ]) {
            current_scene.on_exit( &self.ctx );
            self.stack.push( next_scene_id );
            next_scene.on_enter( &self.ctx );
          }
        }
      },
      SceneEvent::Pop => {
        if let Some( scene ) = self.stack.last().and_then( |current| self.scenes.get_mut( current ) ) {
          scene.on_exit( &self.ctx );
          self.stack.pop();

          if let Some( parent_scene ) = self.stack.last().and_then( |parent_id| self.scenes.get_mut( parent_id ) ) {
            parent_scene.on_enter( &self.ctx );
          }
        }
      }
      SceneEvent::Switch( next_scene_id ) => {
        if self.scenes.contains_key( next_scene_id ) {
          // Call `Scene::on_exit` for each scene in the stack, in reverse
          for scene_id in self.stack.iter().rev() {
            if let Some( scene ) = self.scenes.get_mut( scene_id ) {
              scene.on_exit( &self.ctx );
            }
          }

          // Clear the stack
          self.stack.clear();

          // Call `Scene::on_enter` for the new scene
          if let Some( next_scene ) = self.scenes.get_mut( next_scene_id ) {
            self.stack.push( next_scene_id );
            next_scene.on_enter( &self.ctx );
          }
        }
      }
      SceneEvent::None => {
        /* Do nothing */
      }
    }
  }
}

impl<T: Actionable + Send + 'static> Drop for Foreground<T> {
  // Restores the original terminal state when `Foreground<T>` is dropped.
  fn drop( &mut self ) { ratatui::restore(); }
}

/// Passed as an argument to all `Scene` trait functions, providing access to
/// internal scene attribute's and capabilities.
pub struct Context<T: Actionable + Send + 'static>
{
  action_tx: mpsc::Sender<T::Action>,
  state: Arc<RwLock<T>>
}

impl<T: Actionable + Send + 'static> Context<T> {
  /// Returns a read-only guard to the `Actionable` state.
  pub fn state( &'_ self ) -> RwLockReadGuard<'_,T> { self.state.blocking_read() }

  /// Sends `action` to the asynchronous `Background<T>` task. Returns true if
  /// the `action` was successfully published.
  pub fn invoke( &mut self, action: T::Action ) -> bool {
    self.action_tx.try_send( action ).is_ok()
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - -  Background Action Handler

pub enum Event {
  Key( KeyEvent ),
  Scene( SceneEvent ),
  Tick( f64 )
}

pub struct Background<T: Actionable + Send + 'static> {
  actionable: Arc<RwLock<T>>,
  action_rx: mpsc::Receiver<T::Action>,
  events_tx: mpsc::Sender<Event>
}

impl<T: Actionable + Send + Sync + 'static> Background<T> {
  pub async fn run( mut self ) {
    let mut tasks = JoinSet::new();

    // TODO
    let cancellation_token = CancellationToken::new();

    // Start task for tick on FPS
    tasks.spawn({
      let cancellation_token = cancellation_token.child_token();
      let event_tx = self.events_tx.clone();

      async move {
        tokio::select!{
          _ = cancellation_token.cancelled() => { println!( "cancellation token exit..." ) }
          _ = async move {
            let fps = Duration::from_secs_f32( 1.0 / DEFAULT_FPS );
            let mut interval = time::interval( fps );
            let start_time = time::Instant::now();

            loop {
              let now = interval.tick().await;
              let _ = event_tx.send( Event::Tick( ( start_time - now ).as_secs_f64() ) ).await;
            }
          } => { }
        }
      }
    });

    // Start task to receive and execute any UI actions
    tasks.spawn({
      let actionable = self.actionable.clone();
      let cancellation_token = cancellation_token.child_token();
      let event_tx = self.events_tx.clone();

      async move {
        tokio::select!{
          _ = cancellation_token.cancelled() => { println!( "cancellation token exit..." ) }
          _ = async move {
            while let Some( action ) = self.action_rx.recv().await {
              let event = actionable.write().await.invoke( action ).await;
              let _ = event_tx.send( Event::Scene( event ) ).await;
            }
          } => { }
        }
      }
    });

    // Start task to capture key events
    tasks.spawn({
      let cancellation_token = cancellation_token.child_token();
      let event_tx = self.events_tx.clone();

      async move {
        tokio::select!{
          _ = cancellation_token.cancelled() => { println!( "cancellation token exit..." ) }
          _ = async move {
            let mut crossterm_events = EventStream::new();

            loop {
              if let Some( Ok( crossterm_event ) ) = crossterm_events.next().fuse().await {
                match crossterm_event {
                  event::Event::Key( key ) => {
                    if key.kind == KeyEventKind::Press {
                      let _ = event_tx.send( Event::Key( key ) ).await;
                    }
                  }
                  _ => { /* no-op */ }
                }
              }
            }
          } => { }
        }
      }
    });
    
    tasks.join_all().await;
  }
}