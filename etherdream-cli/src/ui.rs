use std::sync::Arc;

use tokio::sync::RwLock;

use crate::scene;
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

impl scene::Actionable for State {
  type Action = Action;

  async fn invoke( &mut self, action: Action ) -> scene::SceneEvent {
    match action {
      Action::Connect( _port ) => {
        if let Some( device ) = self.get_device_mut() {
          let _ = device.connect().await;
          return scene::SceneEvent::None;
        }

        scene::SceneEvent::Pop
      },
      Action::SelectDevice( device_id ) => {
        self.set_device( Some( device_id ) );
        scene::SceneEvent::Switch( DEVICE_ID )
      },
      Action::DeselectDevice => {
        self.set_device( None );
        scene::SceneEvent::Switch( LIST_ID )
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene Definitions

pub fn build_scenes( state: Arc<RwLock<State>> ) -> Result<( scene::Foreground<State>, scene::Background<State> ),scene::BuilderError> {
  let mut builder = scene::Builder::new( state );

  builder.add_scene( LIST_ID, Box::new( list::ListScene::new() ) );
  builder.add_scene( DEVICE_ID, Box::new( device::DeviceScene::default() ) );
  builder.add_scene( CONNECT_ID, Box::new( connect::ConnectScene::new() ) );

  builder.build()
}