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
pub enum SceneEvent {
  Connect( SocketAddr ),    // Connect to a device
  Disconnect( SocketAddr ), // Disconnect from a device
  Play( SocketAddr ),       // Start playing point data for a device
  Exit,                     // The scene should be exited
  Handled,                  // The event was handled internally by the scene
  NotHandled,               // The event was not handled by the scene
  Select( SocketAddr )      // A device was selected
}

// All scenes must implement this trait
pub trait IsScene {
  fn on_key_down( &mut self, ctx: &Context, key: KeyCode ) -> SceneEvent;
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