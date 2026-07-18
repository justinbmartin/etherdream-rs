use std::cell::RefCell;
use std::rc::Rc;

use crate::device::DeviceMap;

pub(crate) struct AssignCurrentDevice{
  device_map: Rc<RefCell<DeviceMap>>,
  device_selected_id: Rc<RefCell<Option<usize>>>
}

impl AssignCurrentDevice {
  pub(crate) fn new( device_map: Rc<RefCell<DeviceMap>>, device_selected_id: Rc<RefCell<Option<usize>>> ) -> Self {
    Self{ device_map, device_selected_id }
  }

  pub fn assign( &mut self, device_id: usize ) {
    if self.device_map.borrow().contains_key( &device_id ) {
      *self.device_selected_id.borrow_mut() = Some( device_id );
    }
  }
}