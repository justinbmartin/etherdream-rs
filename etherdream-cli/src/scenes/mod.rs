use std::collections::HashMap;
use std::sync::LazyLock;

mod device;
pub(crate) mod list;

// Export our scenes
pub use device::DeviceScene;
pub use list::ListScene;

use super::scene;

#[derive( Clone, Copy, Eq, Hash, PartialEq )]
pub enum Action {
  OnListSelect( usize )
}

#[derive( Clone, Copy, Eq, Hash, PartialEq )]
pub enum SceneAction {
  Select( usize )
}

pub(crate) fn make_scenes() -> scene::SceneController {
  let mut builder = scene::Builder::new( Box::new( ListScene::default() ) );
  builder.add_scene( Box::new( device::ConnectFormScene::default() ) );

  builder.build()
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