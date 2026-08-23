use crate::read_only::ReadOnlyArc;
use crate::scene::SceneDefinitionContext;
use crate::state::{ ReadOnlyDevice, State };

mod connect;
mod list;
mod device;

pub const CONNECT_ID: &str  = "connect";
pub const DEVICE_ID: &str   = "device";
pub const LIST_ID: &str     = "list";

pub fn build_scenes( ctx: &mut SceneDefinitionContext<State> ) {
  let device = ReadOnlyDevice::new( ctx.state().clone() );

  ctx.add_scene( LIST_ID, Box::new( list::ListScene::new( ctx.state().read_only_device_map() ) ) );
  ctx.add_scene( DEVICE_ID, Box::new( device::DeviceScene::new( device.clone() ) ) );
  ctx.add_scene( CONNECT_ID, Box::new( connect::ConnectScene::new( device.clone() ) ) );
}