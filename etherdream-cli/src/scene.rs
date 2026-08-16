use std::collections::HashMap;

use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub enum Event {
  Push( &'static str ),
  Pop,
  Switch( &'static str ),
  None
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene

pub trait Scene<A: ActionHandler> {
  fn on_enter( &mut self ) {}
  fn on_exit( &mut self ) {}
  fn on_key_down( &mut self, _key: KeyEvent ) -> bool { false }
  fn on_update( &mut self, _ctx: &mut UpdateContext<A> ) { }
  fn on_draw( &mut self, area: Rect, buf: &mut Buffer );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Action Handler

pub trait ActionHandler {
  type Event;

  fn invoke( &mut self, action: Self::Event ) -> Event;
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Update Context

pub struct UpdateContext<'a, A: ActionHandler> {
  handler: &'a mut A,
  next_action: Event
}

impl<'a, A: ActionHandler> UpdateContext<'a, A> {
  pub fn invoke( &mut self, action: A::Event ) -> Event {
    self.handler.invoke( action )
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct Builder<A: ActionHandler> {
  current: Option<&'static str>,
  scenes: HashMap<&'static str, Box<dyn Scene<A>>>
}

impl<A: ActionHandler> Builder<A> {
  pub fn new() -> Self {
    Self{
      current: None,
      scenes: HashMap::new()
    }
  }

  /// Adds a scene to the builder.
  pub fn add_scene( &mut self, id: &'static str, scene: Box<dyn Scene<A>> ) -> bool {
    self.scenes.insert( id, scene );
    if self.current.is_none() { self.current = Some( id ) }
    true
  }

  pub fn build( self, handler: A ) -> Controller<A> {
    Controller::<A>{
      handler,
      scenes: self.scenes,
      stack: Vec::with_capacity( 10 )
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct Controller<A: ActionHandler> {
  handler: A,
  scenes: HashMap<&'static str, Box<dyn Scene<A>>>,
  stack: Vec<&'static str>
}

impl<A: ActionHandler> Controller<A> {

  /// ...
  pub fn key_down( &mut self, key: KeyEvent ) -> bool {
    if let Some( scene ) = self.scenes.get_mut( *self.stack.last().unwrap() ) {
      scene.on_key_down( key )
    } else {
      false
    }
  }

  /// ...
  pub fn update( &mut self ) {
    if let Some( scene ) = self.scenes.get_mut( *self.stack.last().unwrap() ) {
      let mut update_ctx = UpdateContext{ handler: &mut self.handler, next_action: Event::None };
      scene.on_update( &mut update_ctx );

      match update_ctx.next_action {
        Event::Push( next_scene ) => {
          scene.on_exit();
          self.stack.push( next_scene );
          self.scenes.get_mut( *self.stack.last().unwrap() ).unwrap().on_enter();
        },
        Event::Pop => {
          scene.on_exit();
          self.stack.pop();
          self.scenes.get_mut( *self.stack.last().unwrap() ).unwrap().on_enter();
        }
        Event::Switch( next_scene ) => {
          scene.on_exit();
          self.stack.clear();
          self.stack.push( next_scene );
          self.scenes.get_mut( *self.stack.last().unwrap() ).unwrap().on_enter();
        }
        Event::None => {}
      }
    }
  }

  /// ...
  pub fn draw( &mut self, area: Rect, buf: &mut Buffer ) {
    if let Some( scene ) = self.scenes.get_mut( *self.stack.last().unwrap() ) {
      scene.on_draw( area, buf );
    }
  }
}