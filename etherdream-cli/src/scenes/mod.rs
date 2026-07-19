use std::cell::{ Ref, RefCell };
use std::rc::Rc;

pub(crate) mod list;
pub(crate) mod device;

// Export our scenes
use crate::device::{ Device, DeviceMap };

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