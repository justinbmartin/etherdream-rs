use std::collections::HashMap;
use std::fmt::Debug;
use std::time::Duration;

use crossterm::event::{ self, EventStream, KeyEvent, KeyEventKind };
use futures::{ FutureExt, StreamExt };
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time;
use tokio_util::sync::CancellationToken;

const DEFAULT_FPS: f32 = 30.0;

#[derive( Debug )]
pub enum SceneEvent {
  Push( &'static str ),
  Pop,
  Switch( &'static str ),
  None
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Actionable

pub trait Actionable {
  /// ...
  type Action: 'static + Send + Debug;

  /// ...
  fn invoke( &mut self, action: Self::Action ) -> impl Future<Output=SceneEvent> + Send;
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene

pub trait Scene<T: Actionable> {
  fn on_enter( &mut self ) { }
  fn on_exit( &mut self ) { }
  fn on_key_down( &mut self, _key: KeyEvent, _ctx: &mut UpdateContext<T> ) -> bool { false }
  fn on_update( &mut self, _ctx: &mut UpdateContext<T> ) { }
  fn on_draw( &mut self, area: Rect, buf: &mut Buffer );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Update Context

pub struct UpdateContext<T: Actionable> {
  action_tx: mpsc::Sender<T::Action>
}

impl<T: Actionable> UpdateContext<T> {
  pub fn invoke( &mut self, action: T::Action ) -> bool {
    let _ = self.action_tx.try_send( action );
    true
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct Builder<T: Actionable + 'static + Send> {
  current: Option<&'static str>,
  scenes: HashMap<&'static str, Box<dyn Scene<T>>>
}

impl<T> Builder<T>
  where T: Actionable + Send + 'static
{
  pub fn new() -> Self {
    Self{
      current: None,
      scenes: HashMap::new()
    }
  }

  /// Adds a scene to the builder.
  pub fn add_scene( &mut self, id: &'static str, scene: Box<dyn Scene<T>> ) -> bool {
    self.scenes.insert( id, scene );
    if self.current.is_none() { self.current = Some( id ) }
    true
  }

  pub fn build( self, action_tx: mpsc::Sender<T::Action> ) -> Controller<T>
  {
    let mut stack = Vec::new();
    stack.push( self.current.unwrap() );

    Controller::<T>{
      action_tx,
      scenes: self.scenes,
      stack
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct Controller<T: Actionable> {
  action_tx: mpsc::Sender<T::Action>,
  scenes: HashMap<&'static str, Box<dyn Scene<T>>>,
  stack: Vec<&'static str>
}

impl<T: Actionable> Controller<T> {
  /// ...
  pub fn key_down( &mut self, key: KeyEvent ) -> bool {
    if let Some( scene ) = self.scenes.get_mut( *self.stack.last().unwrap() ) {
      let mut update_ctx = UpdateContext{ action_tx: self.action_tx.clone() };
      scene.on_key_down( key, &mut update_ctx )
    } else {
      false
    }
  }

  /// ...
  pub fn update( &mut self ) {
    if let Some( scene ) = self.scenes.get_mut( *self.stack.last().unwrap() ) {
      let mut update_ctx = UpdateContext{ action_tx: self.action_tx.clone() };
      scene.on_update( &mut update_ctx );
    }
  }

  /// ...
  pub fn draw( &mut self, area: Rect, buf: &mut Buffer ) {
    if let Some( scene ) = self.scenes.get_mut( *self.stack.last().unwrap() ) {
      scene.on_draw( area, buf );
    }
  }

  pub fn on_event( &mut self, event: SceneEvent ) {
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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Event Context

pub enum Event {
  Key( KeyEvent ),
  Scene( SceneEvent ),
  Tick( f64 )
}

pub fn make_event_server<T: Actionable + Send + 'static>( actionable: T ) -> ( mpsc::Sender<T::Action>, mpsc::Receiver<Event>, EventServer<T> ) {
  let ( action_tx, action_rx ) = mpsc::channel( 16 );
  let ( event_tx, event_rx ) = mpsc::channel( 1024 );

  ( action_tx, event_rx, EventServer{ actionable, action_rx, event_tx })
}

pub struct EventServer<T: Actionable + Send + 'static> {
  actionable: T,
  action_rx: mpsc::Receiver<T::Action>,
  event_tx: mpsc::Sender<Event>
}

impl<T: Actionable + Send + 'static> EventServer<T> {
  pub async fn run( mut self ) {
    let mut tasks = JoinSet::new();

    // TODO
    let cancellation_token = CancellationToken::new();

    tasks.spawn({
      let cancellation_token = cancellation_token.child_token();
      let event_tx = self.event_tx.clone();

      // Start interval tick for FPS
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

    tasks.spawn({
      let cancellation_token = cancellation_token.child_token();
      let event_tx = self.event_tx.clone();

      // Start interval tick for FPS
      async move {
        tokio::select!{
          _ = cancellation_token.cancelled() => { println!( "cancellation token exit..." ) }
          _ = async move {
            while let Some( action ) = self.action_rx.recv().await {
              let event = self.actionable.invoke( action ).await;
              let _ = event_tx.send( Event::Scene( event ) ).await;
            }
          } => { }
        }
      }
    });

    // Start task to capture key events
    tasks.spawn({
      let cancellation_token = cancellation_token.child_token();
      let event_tx = self.event_tx.clone();

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