use std::collections::HashMap;
use std::pin::Pin;

use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

type OnKeyDownFn = Box<dyn FnMut( KeyEvent ) -> Pin<Box<dyn Future<Output = bool>>>>;
type OnUpdateFn<T> = Box<dyn FnMut() -> Pin<Box<dyn Future<Output = Event<T>>>>>;
type OnRenderFn = Box<dyn Fn( Rect, &mut Buffer )>;
type SceneMap<E> = HashMap<&'static str,SceneDefinition<E>>;

pub enum Event<T> {
  NoChange,
  Change( T ),
  Push( T ),
  Pop
}

pub struct SceneDefinition<T> {
  pub on_key_down: Option<OnKeyDownFn>,
  pub on_update: Option<OnUpdateFn<T>>,
  pub on_render: OnRenderFn
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
  pub fn add_scene( &mut self, name: &'static str, scene_definition: SceneDefinition<E> ) {
    self.scenes.insert( name, scene_definition );
    if self.current.is_none() { self.current = Some( name ) }
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
    //self.scenes.get_mut( &self.current ).unwrap().on_exit();
    self.current = scene_id;
    //self.scenes.get_mut( &self.current ).unwrap().on_enter();
  }

  /// ...
  pub async fn key_down( &mut self, key: KeyEvent ) -> bool {
    if let Some( scene ) = self.scenes.get_mut( self.current ) {
      if let Some( callback ) = &mut scene.on_key_down {
        return callback( key ).await
      }
    }

    false
  }

  ///...
  pub async fn update( &mut self ) -> Event<E> {
    if let Some( scene ) = self.scenes.get_mut( self.current ) {
      if let Some( callback ) = &mut scene.on_update {
        return callback().await;
      }
    }

    Event::NoChange
  }

  /// ...
  pub fn render( &mut self, area: Rect, buf: &mut Buffer ) {
    if let Some( scene ) = self.scenes.get_mut( self.current ) {
      ( scene.on_render )( area, buf );
    }
  }
}