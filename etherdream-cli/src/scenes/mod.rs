use crate::device::{ ReadOnlyDeviceMap, ScopedDevice };
use crate::scene;
use crate::state::{ self, State };

mod connect;
mod list;
mod device;

pub const CONNECT_ID: &str = "connect";
pub const DEVICE_ID: &str = "device";
pub const LIST_ID: &str = "list";

pub fn build( builder: &mut scene::Builder<State> ) {
  {
    let device_map = ReadOnlyDeviceMap::new( builder.state().device_map.clone() );
    builder.add_scene( LIST_ID, Box::new( list::ListScene::new( device_map ) ) );
  }

  {
    let scoped_device = ScopedDevice::new( builder.state().device_id.clone(), builder.state().device_map.clone() );
    builder.add_scene( DEVICE_ID, Box::new( device::DeviceScene::new( scoped_device ) ) );
  }

  {
    let scoped_device = ScopedDevice::new( builder.state().device_id.clone(), builder.state().device_map.clone() );
    builder.add_scene( CONNECT_ID, Box::new( connect::ConnectScene::new( scoped_device ) ) );
  }
}