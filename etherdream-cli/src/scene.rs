use std::collections::HashMap;
use std::fmt::Debug;
use std::time::Duration;

use crossterm::event::{ self, EventStream, KeyCode, KeyEvent, KeyEventKind };
use futures::{ FutureExt, StreamExt };
use ratatui::{ buffer::Buffer, layout::{ Constraint, Layout, Rect }, widgets::{ Paragraph, Widget } };
use tokio::{ sync::mpsc, task::JoinSet, time };
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

pub trait Scene<T> {
  fn on_enter( &mut self ) { }
  fn on_exit( &mut self ) { }
  fn on_key_down( &mut self, _key: KeyEvent, _ctx: &mut UpdateContext<T> ) -> bool { false }
  fn on_update( &mut self, _ctx: &mut UpdateContext<T> ) { }
  fn on_draw( &mut self, area: Rect, buf: &mut Buffer );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Init

pub fn init<T,U>( build_scene_fn: T, mut state: U ) -> ( UI<U::Action>, Server<U> )
where T: Fn( &mut SceneDefinitionContext<U> ),
      U: Actionable + Send + 'static
{
  let ( action_tx, action_rx ) = mpsc::channel::<U::Action>( 16 );
  let ( events_tx, events_rx ) = mpsc::channel::<Event>( 1024 );

  // Call user-provided scene description builder
  let mut ctx = SceneDefinitionContext::new( &mut state );
  build_scene_fn( &mut ctx );

  //
  let mut stack = Vec::new();
  stack.push( ctx.current.unwrap() );

  (
    UI{
      events_rx,
      scenes: ctx.scenes,
      stack,
      update_ctx: UpdateContext{ action_tx }
    },
    Server{
      action_rx,
      actionable: state,
      events_tx
    }
  )
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene Builder

pub struct SceneDefinitionContext<'a, T: Actionable> {
  current: Option<&'static str>,
  scenes: HashMap<&'static str, Box<dyn Scene<T::Action>>>,
  state: &'a mut T
}

impl<'a,T: Actionable> SceneDefinitionContext<'a,T> {
  pub fn new( state: &'a mut T ) -> Self {
    Self{
      current: None,
      scenes: HashMap::new(),
      state
    }
  }

  /// Adds a scene to the builder.
  pub fn add_scene( &mut self, id: &'static str, scene: Box<dyn Scene<T::Action>> ) -> bool {
    self.scenes.insert( id, scene );
    if self.current.is_none() { self.current = Some( id ) }
    true
  }

  /// Returns an immutable reference to the root state.
  pub fn state( &self ) -> &T { self.state }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct UI<T> {
  events_rx: mpsc::Receiver<Event>,
  scenes: HashMap<&'static str, Box<dyn Scene<T>>>,
  stack: Vec<&'static str>,
  update_ctx: UpdateContext<T>
}

impl<T> UI<T> {
  pub fn run( &mut self ) {
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

    ratatui::restore();
  }

  /// ...
  fn key_down( &mut self, key: KeyEvent ) -> bool {
    if let Some( scene ) = self.scenes.get_mut( *self.stack.last().unwrap() ) {
      scene.on_key_down( key, &mut self.update_ctx )
    } else {
      false
    }
  }

  /// ...
  fn update( &mut self ) {
    if let Some( scene ) = self.scenes.get_mut( *self.stack.last().unwrap() ) {
      scene.on_update( &mut self.update_ctx );
    }
  }

  /// ...
  fn draw( &mut self, area: Rect, buf: &mut Buffer ) {
    if let Some( scene ) = self.scenes.get_mut( *self.stack.last().unwrap() ) {
      scene.on_draw( area, buf );
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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Update Context

pub struct UpdateContext<T> {
  action_tx: mpsc::Sender<T>
}

impl<T> UpdateContext<T> {
  pub fn invoke( &mut self, action: T ) -> bool {
    self.action_tx.try_send( action ).is_ok()
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Event Context

pub enum Event {
  Key( KeyEvent ),
  Scene( SceneEvent ),
  Tick( f64 )
}

pub struct Server<T: Actionable> {
  actionable: T,
  action_rx: mpsc::Receiver<T::Action>,
  events_tx: mpsc::Sender<Event>
}

impl<T: Actionable + Send + 'static> Server<T> {
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
      let cancellation_token = cancellation_token.child_token();
      let event_tx = self.events_tx.clone();

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