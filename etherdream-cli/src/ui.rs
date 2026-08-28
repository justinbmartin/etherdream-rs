use std::sync::Arc;

use tokio::sync::RwLock;

use crate::scene::{ Actionable, ScenesDefinition, SceneEvent };
use crate::state::State;

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
        let device_id = self.device_id().unwrap();

        if let Some( device ) = self.device_by_id_mut( device_id ) {
          let _ = device.connect().await;
          return SceneEvent::None;
        }

        SceneEvent::Pop
      },
      Action::SelectDevice( device_id ) => {
        self.set_device_id( Some( device_id ) );
        SceneEvent::Switch( DEVICE_ID )
      },
      Action::DeselectDevice => {
        self.set_device_id( None );
        SceneEvent::Switch( LIST_ID )
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene Definitions

pub fn build_scenes( state: Arc<RwLock<State>> ) -> ScenesDefinition<State> {
  let mut def = ScenesDefinition::new( state );

  def.add_scene( LIST_ID, Box::new( list::ListScene::new() ) );
  def.add_scene( DEVICE_ID, Box::new( device::DeviceScene::default() ) );
  def.add_scene( CONNECT_ID, Box::new( connect::ConnectScene::new() ) );

  def
}