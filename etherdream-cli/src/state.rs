use std::sync::Arc;

use tokio::sync::Mutex;

use crate::device::DeviceMap;
use crate::scene;
use crate::scenes;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

#[derive( Debug )]
pub enum Action {
  Connect( u16 ),
  SelectDevice( usize ),
  DeselectDevice
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  State

pub struct State {
  pub device_id: Arc<Mutex<Option<usize>>>,
  pub device_map: Arc<Mutex<DeviceMap>>
}

impl State {
  pub fn new( device_map: Arc<Mutex<DeviceMap>> ) -> Self {
    Self{ device_id: Arc::new( Mutex::new( None::<usize> ) ), device_map }
  }
}

impl Clone for State {
  fn clone( &self ) -> Self {
    Self{
      device_id: self.device_id.clone(),
      device_map: self.device_map.clone()
    }
  }
}

impl scene::Actionable for State {
  type Action = Action;

  async fn invoke( &mut self, action: Action ) -> scene::SceneEvent {
    match action {
      Action::Connect( _port ) => {
        let device_id = self.device_id.lock().await.unwrap();

        if let Some( device ) = self.device_map.lock().await.get_mut( device_id ) {
          let _ = device.connect().await;
          return scene::SceneEvent::None;
        }

        scene::SceneEvent::Pop
      },
      Action::SelectDevice( id ) => {
        *self.device_id.lock().await = Some( id );
        scene::SceneEvent::Switch( scenes::DEVICE_ID )
      },
      Action::DeselectDevice => {
        *self.device_id.lock().await = None;
        scene::SceneEvent::Switch( scenes::LIST_ID )
      }
    }
  }
}