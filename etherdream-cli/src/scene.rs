use std::collections::HashMap;

use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

type SceneMap<Ctx> = HashMap<&'static str,Box<dyn Scene<Ctx>>>;

#[derive( PartialEq )]
pub enum Event {
  Change( &'static str ),
  Handled,             // The event was handled internally by the scene
  NotHandled           // The event was not handled by the scene
}

pub trait Scene<Ctx> {
  /// Called once before any call to `on_render`. (Optional)
  fn on_enter( &mut self ) { /* no-op */ }

  /// Called once after the last call to `on_render`. (Optional)
  fn on_exit( &mut self ) { /* no-op */ }

  /// Called each time a key-press is registered if this scene is active. (Optional)
  fn on_key_down( &mut self, _ctx: &Ctx, _key: KeyEvent ) -> Event {
    Event::NotHandled
  }

  /// Called on each frame if this scene is active. (Required)
  fn on_render( &mut self, ctx: &Ctx, area: Rect, buf: &mut Buffer );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct Builder<Ctx> {
  active: &'static str,
  scenes: SceneMap<Ctx>
}

impl<Ctx> Builder<Ctx> {
  pub fn new() -> Self {
    Self{
      active: "",
      scenes: SceneMap::<Ctx>::new()
    }
  }

  /// Adds a scene to the builder.
  pub fn add_scene( &mut self, name: &'static str, scene: Box<dyn Scene<Ctx>> ) {
    self.scenes.insert( name, scene );
  }

  pub fn build( self ) -> Controller<Ctx> {
    Controller::<Ctx>{
      current: self.active,
      scenes: self.scenes
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct Controller<Ctx> {
  current: &'static str,
  scenes: SceneMap<Ctx>
}

impl<Ctx> Controller<Ctx> {
  pub fn current_scene( &self ) -> &str { &self.current }

  pub fn change( &mut self, scene_id: &str ) {
    // TODO: validate
    self.scenes.get_mut( &self.current ).unwrap().on_exit();
    self.current = scene_id;
    self.scenes.get_mut( &self.current ).unwrap().on_enter();
  }
}