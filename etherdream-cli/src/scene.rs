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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Actionable

pub trait Actionable {
  /// ...
  type Action;

  /// ...
  fn invoke( &mut self, action: Self::Action ) -> Event;
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene

pub trait Scene<T: Actionable> {
  fn on_enter( &mut self ) {}
  fn on_exit( &mut self ) {}
  fn on_key_down( &mut self, _key: KeyEvent ) -> bool { false }
  fn on_update( &mut self, _ctx: &mut UpdateContext<T> ) { }
  fn on_draw( &mut self, area: Rect, buf: &mut Buffer );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Update Context

pub struct UpdateContext<'a, T: Actionable> {
  action: &'a mut T,
  next_action: Event
}

impl<'a, T: Actionable> UpdateContext<'a, T> {
  pub fn invoke( &mut self, action: T::Action ) -> Event {
    self.action.invoke( action )
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct Builder<T: Actionable> {
  action: T,
  current: Option<&'static str>,
  scenes: HashMap<&'static str, Box<dyn Scene<T>>>
}

impl<T: Actionable> Builder<T> {
  pub fn new( action: T ) -> Self {
    Self{
      action,
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

  pub fn build( self ) -> Controller<T> {
    Controller::<T>{
      action: self.action,
      scenes: self.scenes,
      stack: Vec::with_capacity( 10 )
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct Controller<T: Actionable> {
  action: T,
  scenes: HashMap<&'static str, Box<dyn Scene<T>>>,
  stack: Vec<&'static str>
}

impl<T: Actionable> Controller<T> {
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
      let mut update_ctx = UpdateContext{ action: &mut self.action, next_action: Event::None };
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