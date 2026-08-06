use std::collections::HashMap;

use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

type SceneMap<E> = HashMap<&'static str,Box<dyn Scene<E>>>;

pub enum Event<T> {
  NoChange,
  Change( T ),
  Push( T ),
  Pop
}

pub trait Scene<SceneEvent> {
  /// Called once before any call to `on_render`. (Optional)
  fn on_enter( &mut self ) { /* no-op */ }

  /// Called once after the last call to `on_render`. (Optional)
  fn on_exit( &mut self ) { /* no-op */ }

  /// Called each time a key-press is registered for this scene. Return true
  /// if the scene handled the key event. Otherwise, return false to bubble the
  /// key event on the graph.
  ///
  /// Should be light-weight. This is called inline with the key-event. Any
  /// processing of that key-event should be handled in your update.
  fn on_key_down( &mut self, _key: KeyEvent ) -> bool {
    false
  }

  /// ...
  fn on_update( &mut self ) -> Event<SceneEvent>;

  /// Called on each frame if this scene is active. (Required)
  fn on_render( &mut self, area: Rect, buf: &mut Buffer );
}

use std::pin::Pin;

type OnKeyDownFn = Box<dyn FnMut( KeyEvent ) -> Pin<Box<dyn Future<Output = bool>>>>;

type OnUpdateFn<T> = Box<dyn FnMut() -> Pin<Box<dyn Future<Output = Event<T>>>>>;

pub struct SceneDefinition<T> {
  pub name: &'static str,
  pub on_key_down: Option<OnKeyDownFn>,
  pub on_update: OnUpdateFn<T>,
  pub on_render: Box<dyn Fn()>
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct Builder<E> {
  current: Option<&'static str>,
  scenes: SceneMap<E>
}

impl<E> Builder<E> {
  pub fn new() -> Self {
    Self{
      current: None,
      scenes: SceneMap::<E>::new()
    }
  }

  /// Adds a scene to the builder.
  pub fn add_scene( &mut self, scene_name: &'static str, scene: Box<dyn Scene<E>> ) {
    self.scenes.insert( scene_name, scene );
    if self.current.is_none() { self.current = Some( scene_name ) }
  }

  pub fn build( self ) -> Controller<E> {
    Controller::<E>{
      current: self.current.unwrap(), // TODO
      scenes: self.scenes
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct Controller<E> {
  current: &'static str,
  scenes: SceneMap<E>
}

impl<E> Controller<E> {
  pub fn _current_scene( &self ) -> &str { &self.current }

  pub fn change( &mut self, scene_id: &'static str ) {
    // TODO: validate
    self.scenes.get_mut( &self.current ).unwrap().on_exit();
    self.current = scene_id;
    self.scenes.get_mut( &self.current ).unwrap().on_enter();
  }

  /// ...
  pub fn key_down( &mut self, key: KeyEvent ) -> bool {
    if let Some( scene ) = self.scenes.get_mut( self.current ) {
      scene.on_key_down( key )
    } else {
      false
    }
  }

  pub fn update( &mut self ) -> Event<E> {
    if let Some( scene ) = self.scenes.get_mut( self.current ) {
      scene.on_update()
    } else {
      Event::NoChange
    }

  }

  /// ...
  pub fn render( &mut self, area: Rect, buf: &mut Buffer ) {
    if let Some( scene ) = self.scenes.get_mut( self.current ) {
      scene.on_render( area, buf );
    }
  }
}