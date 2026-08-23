use crate::scene::SceneDefinitionContext;
use crate::state::{ ScopedDevice, State };

mod connect;
mod list;
mod device;

pub const CONNECT_ID: &str  = "connect";
pub const DEVICE_ID: &str   = "device";
pub const LIST_ID: &str     = "list";

#[derive( Debug )]
pub enum Action {
  Connect( u16 ),
  SelectDevice( usize ),
  DeselectDevice
}

pub fn build( ctx: &mut SceneDefinitionContext<State> ) {
  let scoped_device = ScopedDevice::new( ctx.state() );

  ctx.add_scene( LIST_ID, Box::new( list::ListScene::new( ctx.state().device_map() ) ) );
  ctx.add_scene( DEVICE_ID, Box::new( device::DeviceScene::new( scoped_device.clone() ) ) );
  ctx.add_scene( CONNECT_ID, Box::new( connect::ConnectScene::new( scoped_device.clone() ) ) );
}