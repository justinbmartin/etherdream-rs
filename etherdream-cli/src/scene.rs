use std::collections::HashMap;

use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

type SceneMap<C,E> = HashMap<&'static str,Box<dyn Scene<C,E>>>;

pub enum Event<T> {
  Handled,
  NotHandled,
  Custom( T )
}

pub trait Scene<SceneContext,SceneEvent> {
  /// Called once before any call to `on_render`. (Optional)
  fn on_enter( &mut self ) { /* no-op */ }

  /// Called once after the last call to `on_render`. (Optional)
  fn on_exit( &mut self ) { /* no-op */ }

  /// Called each time a key-press is registered for this scene. Return true
  /// if the scene handled the key event. Otherwise, return false to bubble the
  /// key event on the graph.
  fn on_key_down( &mut self, _ctx: &SceneContext, _key: KeyEvent ) -> Event<SceneEvent> {
    Event::NotHandled
  }

  /// Called on each frame if this scene is active. (Required)
  fn on_render( &mut self, ctx: &SceneContext, area: Rect, buf: &mut Buffer );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct Builder<C,E> {
  current: Option<&'static str>,
  scenes: SceneMap<C,E>
}

impl<C,E> Builder<C,E> {
  pub fn new() -> Self {
    Self{
      current: None,
      scenes: SceneMap::<C,E>::new()
    }
  }

  /// Adds a scene to the builder.
  pub fn add_scene( &mut self, scene_name: &'static str, scene: Box<dyn Scene<C,E>> ) {
    self.scenes.insert( scene_name, scene );
    if self.current.is_none() { self.current = Some( scene_name ) }
  }

  pub fn build( self ) -> Controller<C,E> {
    Controller::<C,E>{
      current: self.current.unwrap(), // TODO
      scenes: self.scenes
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct Controller<C,E> {
  current: &'static str,
  scenes: SceneMap<C,E>
}

impl<C,E> Controller<C,E> {
  pub fn _current_scene( &self ) -> &str { &self.current }

  pub fn change( &mut self, scene_id: &'static str ) {
    // TODO: validate
    self.scenes.get_mut( &self.current ).unwrap().on_exit();
    self.current = scene_id;
    self.scenes.get_mut( &self.current ).unwrap().on_enter();
  }

  /// ...
  pub fn key_down( &mut self, ctx: &mut C, key: KeyEvent ) -> Event<E> {
    if let Some( scene ) = self.scenes.get_mut( self.current ) {
      scene.on_key_down( ctx, key )
    } else {
      Event::NotHandled
    }
  }

  /// ...
  pub fn render( &mut self, ctx: &C, area: Rect, buf: &mut Buffer ) {
    if let Some( scene ) = self.scenes.get_mut( self.current ) {
      scene.on_render( ctx, area, buf );
    }
  }
}