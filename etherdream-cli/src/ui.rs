use crate::read_only::ReadOnlyArc;
use crate::scene::{ Actionable, SceneDefinitionContext, SceneEvent };
use crate::state::{ ReadOnlyDevice, State };

mod connect;
mod list;
mod device;

pub const CONNECT_ID: &str  = "connect";
pub const DEVICE_ID: &str   = "device";
pub const LIST_ID: &str     = "list";

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Actions

#[derive( Debug )]
pub enum Action {
  Connect( u16 ),
  SelectDevice( usize ),
  DeselectDevice
}

impl Actionable for State {
  type Action = Action;

  async fn invoke( &mut self, action: Action ) -> SceneEvent {
    match action {
      Action::Connect( _port ) => {
        let device_id = self.device_id().read().await.unwrap();

        if let Some( device ) = self.device_map().write().await.get_mut( device_id ) {
          let _ = device.connect().await;
          return SceneEvent::None;
        }

        SceneEvent::Pop
      },
      Action::SelectDevice( id ) => {
        *self.device_id().write().await = Some( id );
        SceneEvent::Switch( DEVICE_ID )
      },
      Action::DeselectDevice => {
        *self.device_id().write().await = None;
        SceneEvent::Switch( LIST_ID )
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene Definitions

pub fn build_scenes( ctx: &mut SceneDefinitionContext<State> ) {
  let device = ReadOnlyDevice::new( ctx.state().clone() );
  let device_map = ReadOnlyArc::new( ctx.state().device_map().clone() );

  ctx.add_scene( LIST_ID, Box::new( list::ListScene::new( device_map ) ) );
  ctx.add_scene( DEVICE_ID, Box::new( device::DeviceScene::new( device.clone() ) ) );
  ctx.add_scene( CONNECT_ID, Box::new( connect::ConnectScene::new( device.clone() ) ) );
}