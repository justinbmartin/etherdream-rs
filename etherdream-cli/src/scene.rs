use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub enum Event<T> {
  Change( T ),
  Noop
}

pub trait Scene<T> {
  fn on_enter( &mut self ) {}
  fn on_exit( &mut self ) {}
  fn on_key_down( &mut self, _key: KeyEvent ) -> bool { false }
  fn on_update( &mut self ) -> Event<T> { Event::Noop }
  fn on_draw( &mut self, area: Rect, buf: &mut Buffer );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct Builder<Action> {
  current: Option<usize>,
  scenes: Vec<Box<dyn Scene<Action>>>
}

impl<T> Builder<T> {
  pub fn new() -> Self {
    Self{
      current: None,
      scenes: Vec::new()
    }
  }

  /// Adds a scene to the builder.
  pub fn add_scene( &mut self, scene: Box<dyn Scene<T>> ) -> usize {
    self.scenes.push( scene );
    let id = self.scenes.len() - 1;
    if self.current.is_none() { self.current = Some( id ) }
    id
  }

  pub fn build( self, handler: Box<dyn Fn( T ) -> usize> ) -> Controller<T> {
    Controller::<T>{
      actions: handler,
      current: self.current.unwrap(), // TODO
      scenes: self.scenes
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct Controller<Action> {
  actions: Box<dyn FnMut( Action ) -> usize>,
  current: usize,
  scenes: Vec<Box<dyn Scene<Action>>>
}

impl<E> Controller<E> {

  /// ...
  pub fn key_down( &mut self, key: KeyEvent ) -> bool {
    if let Some( scene ) = self.scenes.get_mut( self.current ) {
      scene.on_key_down( key )
    } else {
      false
    }
  }

  /// ...
  pub fn update( &mut self ) {
    if let Some( scene ) = self.scenes.get_mut( self.current ) {
      match scene.on_update() {
        Event::Change( action ) => {
          scene.on_exit();
          self.current = ( self.actions )( action );
          self.scenes.get_mut( self.current ).unwrap().on_enter();
        },
        Event::Noop => {}
      };
    }
  }

  /// ...
  pub fn draw( &mut self, area: Rect, buf: &mut Buffer ) {
    if let Some( scene ) = self.scenes.get_mut( self.current ) {
      scene.on_draw( area, buf );
    }
  }
}