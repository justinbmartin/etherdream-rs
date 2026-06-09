use std::cell::{ Ref, RefCell };
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::device::{ Device, DeviceMap };

#[derive( PartialEq )]
pub enum SceneEvent {
  Connect( usize ),    // Connect to a device
  Disconnect( usize ), // Disconnect from a device
  Play( usize ),       // Start playing point data for a device
  Exit,                // The scene should be exited
  Handled,             // The event was handled internally by the scene
  NotHandled,          // The event was not handled by the scene
  Select( usize )      // A device was selected
}

pub trait Scene {
  /// Called once when the scene is entered. (Optional)
  fn on_scene_enter( &mut self ) { /* no-op */ }

  /// Called once when the scene is exited. (Optional)
  fn on_scene_exit( &mut self ) { /* no-op */ }

  /// Called each time a key-press is registered. (Optional)
  fn on_key_down( &mut self, _ctx: &SceneContext, _key: KeyEvent ) -> SceneEvent {
    SceneEvent::NotHandled
  }

  /// Called on each frame. (Required)
  fn render( &mut self, ctx: &SceneContext, area: Rect, buf: &mut Buffer );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene Context

/// Read-only context provided to Scene-trait implementation functions.
pub struct SceneContext {
  device_map: Rc<RefCell<DeviceMap>>,
  device_selected_id: Rc<RefCell<Option<usize>>>
}

impl SceneContext {
  pub fn new( device_map: Rc<RefCell<DeviceMap>>, device_selected_id: Rc<RefCell<Option<usize>>> ) -> Self {
    Self{ device_map, device_selected_id }
  }

  // Returns a read-only reference to the list of discovered device infos
  pub fn device_map( &'_ self ) -> Ref<'_, DeviceMap> {
    self.device_map.borrow()
  }

  // Returns the selected device, if one is set
  pub fn selected_device( &'_ self ) -> Option<Ref<'_, Device>> {
    if let Some( id ) = *self.device_selected_id.borrow() {
      Ref::filter_map( self.device_map.borrow(), |dm|{ dm.get( id ) } ).ok()
    } else {
      None
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene Manager + Builder

pub struct SceneManagerBuilder<Key>
  where Key: Copy + Eq + Hash + PartialEq
{
  scene: Key,
  scenes: HashMap<Key,Box<dyn Scene>>
}

impl<Key> SceneManagerBuilder<Key>
  where Key: Copy + Eq + Hash + PartialEq
{
  pub fn new( key: Key, scene: Box<dyn Scene> ) -> Self {
    let mut scenes = HashMap::new();
    scenes.insert( key, scene );

    Self{
      scene: key,
      scenes
    }
  }

  pub fn add_scene( mut self, key: Key, scene: Box<dyn Scene> ) -> Self {
    self.scenes.insert( key, scene );
    self
  }

  pub fn build( self ) -> SceneManager<Key> {
    SceneManager::<Key>{
      scene: self.scene,
      scenes: self.scenes
    }
  }
}

pub struct SceneManager<Key>
  where Key: Copy + Eq + Hash + PartialEq
{
  scene: Key,
  scenes: HashMap<Key,Box<dyn Scene>>
}

impl<Key> SceneManager<Key>
  where Key: Copy + Eq + Hash + PartialEq
{
  pub fn current_scene( &mut self ) -> &mut Box<dyn Scene> {
    self.scenes.get_mut( &self.scene ).unwrap()
  }

  pub fn current_scene_key( &self ) -> Key { self.scene }

  pub fn set_scene( &mut self, key: Key ) {
    self.current_scene().on_scene_exit();
    self.scene = key;
    self.current_scene().on_scene_enter();
  }
}