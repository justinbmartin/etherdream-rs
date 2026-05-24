mod info;
mod list;

use std::cell::{ Ref, RefCell };
use std::net::SocketAddr;
use std::rc::Rc;

use crossterm::event::KeyCode;
use ratatui::prelude::*;

use crate::device::{ Device, DeviceMap };

// Export our scenes
pub use info::InfoScene;
pub use list::ListScene;

#[derive( Eq, Hash, PartialEq )]
pub enum Scene { List, Info }

// Return values from scene key events
#[derive( PartialEq )]
pub enum SceneEvent<'a> {
  Exit,           // The scene should be exited
  Handled,        // The event was handled internally by the scene
  NotHandled,     // The event was not handled by the scene
  Select( &'a SocketAddr ) // A device was selected
}

// All scenes must implement this trait
pub trait IsScene {
  fn on_key_press( &'_ mut self, ctx: &mut Context, key: KeyCode ) -> SceneEvent<'_>;
  fn render( &mut self, ctx: &Context, area: Rect, buf: &mut Buffer );
}

// Shared read-only scene data
pub struct Context {
  device_map: Rc<RefCell<DeviceMap>>,
  device_selected_id: Rc<RefCell<Option<SocketAddr>>>
}

impl Context {
  pub fn new( device_map: Rc<RefCell<DeviceMap>>, device_selected_id: Rc<RefCell<Option<SocketAddr>>> ) -> Self {
    Self{ device_map, device_selected_id }
  }

  // Returns a read-only reference to the list of discovered device infos
  pub fn device_map( &'_ self ) -> Ref<'_, DeviceMap> {
    self.device_map.borrow()
  }

  // Returns the selected device, if one is set
  pub fn selected_device( &'_ self ) -> Option<Ref<'_, Device>> {
    if let Some( id ) = *self.device_selected_id.borrow() {
      Ref::filter_map( self.device_map.borrow(), |dm|{ dm.get( &id ) } ).ok()
    } else {
      None
    }
  }
}