use std::net::SocketAddr;
use std::cell::{ Ref, RefCell };
use std::collections::HashMap;
use std::rc::Rc;

use crossterm::event::{ KeyCode };
use ratatui::prelude::*;

use crate::device::Device;

// List of scenes this application contains.
#[derive( Eq, Hash, PartialEq )]
pub enum Scene { List, Info }

// Return values from scene key events
#[derive( PartialEq )]
pub enum SceneEvent {
  Exit,           // The scene should be exited
  Handled,        // The event was handled internally by the scene
  NotHandled,     // The event was not handled by the scene
  Select( SocketAddr ) // A device was selected
}

// All scenes must implement this trait
pub trait IsScene {
  fn on_key_press( &mut self, key: KeyCode ) -> SceneEvent;
  fn render( &mut self, area: Rect, buf: &mut Buffer );
}

// Shared read-only scene data
pub struct SceneData {
  device_map: Rc<RefCell<HashMap<SocketAddr,Device>>>,
  device_selected_id: Rc<RefCell<Option<SocketAddr>>>
}

impl SceneData {
  pub fn new( device_map: Rc<RefCell<HashMap<SocketAddr,Device>>>, device_selected_id: Rc<RefCell<Option<SocketAddr>>> ) -> Self {
    Self{ device_map, device_selected_id }
  }

  // Returns a read-only reference to the list of discovered device infos
  pub fn device_map( &'_ self ) -> Ref<'_, HashMap<SocketAddr,Device>> {
    self.device_map.borrow()
  }

  // Returns the selected device index, if one is set
  pub fn device_selected_id( &self ) -> Option<SocketAddr> {
    *self.device_selected_id.borrow()
  }
}

impl Clone for SceneData {
  fn clone( &self ) -> Self {
    SceneData{
      device_map: self.device_map.clone(),
      device_selected_id: self.device_selected_id.clone()
    }
  }
}