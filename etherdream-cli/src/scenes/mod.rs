use crate::scene;
use crate::state::{ self, ScopedDevice, State };

mod connect;
mod list;
mod device;

pub const CONNECT_ID: &str = "connect";
pub const DEVICE_ID: &str = "device";
pub const LIST_ID: &str = "list";

pub fn build( builder: &mut scene::Builder<State> ) {
  let scoped_device = ScopedDevice::new( builder.state() );

  builder.add_scene( LIST_ID, Box::new( list::ListScene::new( builder.state().device_map() ) ) );
  builder.add_scene( DEVICE_ID, Box::new( device::DeviceScene::new( scoped_device.clone() ) ) );
  builder.add_scene( CONNECT_ID, Box::new( connect::ConnectScene::new( scoped_device.clone() ) ) );
}