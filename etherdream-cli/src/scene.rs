use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{ self, EventStream, KeyCode, KeyEvent, KeyEventKind };
use futures::{ FutureExt, StreamExt };
use ratatui::{ buffer::Buffer, layout::{ Constraint, Layout, Rect }, widgets::{ Paragraph, Widget } };
use tokio::{ sync::{ mpsc, RwLock, RwLockReadGuard }, task::JoinSet, time };
use tokio_util::sync::CancellationToken;

const DEFAULT_FPS: f32 = 30.0;

#[derive( Debug )]
pub enum SceneEvent {
  /// No action.
  None,
  /// Pushes a new scene on to the stack. Useful for modals.
  Push( &'static str ),
  /// Pops the current scene from the stack.
  Pop,
  /// Swaps the current scene with a new scene.
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
  fn on_enter( &mut self ) { }

  /// Called once, after the last update and draw, when the scene is exited.
  fn on_exit( &mut self ) { }

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
          if ! self.key_down( key ) {
            match key.code {
              KeyCode::Char( 'q' ) | KeyCode::Esc => { break; },
              _ => { }
            }
          }
        },
        Event::Tick( _time ) => {
          let _ = self.update();

          let _ = terminal.draw(| frame |{
            let main_layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
            let [ body_area, footer_area ] = frame.area().layout( &main_layout );

            // Main > Body
            self.draw( body_area, frame.buffer_mut() );

            // Main > Footer
            Paragraph::new( "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." )
              .centered()
              .render( footer_area, frame.buffer_mut() );
          });
        }
        Event::Scene( event ) => {
          self.on_event( event )
        }
      }
    }
  }

  /// ...
  fn key_down( &mut self, key: KeyEvent ) -> bool {
    if let Some( scene ) = self.stack.last().and_then( |current| self.scenes.get_mut( current ) ) {
      scene.on_key_down( key, &mut self.ctx )
    } else {
      false
    }
  }

  /// ...
  fn update( &mut self ) {
    if let Some( scene ) = self.scenes.get_mut( *self.stack.last().unwrap() ) {
      scene.on_update( &mut self.ctx );
    }
  }

  /// ...
  fn draw( &mut self, area: Rect, buf: &mut Buffer ) {
    if let Some( scene ) = self.scenes.get_mut( *self.stack.last().unwrap() ) {
      scene.on_draw( area, buf, &self.ctx );
    }
  }

  fn on_event( &mut self, event: SceneEvent ) {
    match event {
      SceneEvent::Push( next_scene ) => {
        if let Some( scene_id ) = self.stack.last() && let Some( scene ) = self.scenes.get_mut( scene_id ) {
          scene.on_exit();
          self.stack.push( next_scene );
          self.scenes.get_mut( *self.stack.last().unwrap() ).unwrap().on_enter();
        }
      },
      SceneEvent::Pop => {
        if let Some( scene_id ) = self.stack.last() && let Some( scene ) = self.scenes.get_mut( scene_id ) {
          scene.on_exit();
          self.stack.pop();
          self.scenes.get_mut( *self.stack.last().unwrap() ).unwrap().on_enter();
        }
      }
      SceneEvent::Switch( next_scene ) => {
        if let Some( scene_id ) = self.stack.last() && let Some( scene ) = self.scenes.get_mut( scene_id ) {
          scene.on_exit();
          self.stack.clear();
          self.stack.push( next_scene );
          self.scenes.get_mut( *self.stack.last().unwrap() ).unwrap().on_enter();
        }
      }
      SceneEvent::None => {}
    }
  }
}

impl<T: Actionable + Send + 'static> Drop for Foreground<T> {
  // Restores the original terminal state when `Foreground<T>` is dropped.
  fn drop( &mut self ) { ratatui::restore(); }
}

pub struct Context<T: Actionable + Send + 'static>
{
  action_tx: mpsc::Sender<T::Action>,
  state: Arc<RwLock<T>>
}

impl<T: Actionable + Send + 'static> Context<T> {
  pub fn state( &'_ self ) -> RwLockReadGuard<'_,T> { self.state.blocking_read() }

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