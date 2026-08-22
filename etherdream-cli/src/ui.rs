use std::sync::{ Arc, Mutex };

use crossterm::event::KeyCode;
use ratatui::{ DefaultTerminal, Frame };
use ratatui::layout::{ Constraint, Layout };
use ratatui::widgets::{ Paragraph, Widget };
use tokio::sync::mpsc::Receiver;

use crate::device;
use crate::scene;
use crate::scenes;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Action

#[derive( Debug )]
pub enum Action {
  Connect( u16 ),
  SelectDevice( usize ),
  DeselectDevice
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

#[derive( Default )]
pub struct State {
  pub device_id: Arc<Mutex<Option<usize>>>,
  pub device_map: Arc<tokio::sync::Mutex<device::DeviceMap>>
}

impl Clone for State {
  fn clone( &self ) -> Self {
    Self{
      device_id: self.device_id.clone(),
      device_map: self.device_map.clone()
    }
  }
}

pub struct UI {
  builder: scene::Builder<State>
}

impl UI {
  pub fn new( state: State ) -> Self {
    
    // Scenes
    let mut builder = scene::Builder::new();

    {
      let device_map = device::ReadOnlyDeviceMap::new( state.device_map.clone() );
      builder.add_scene( scenes::list::ID, Box::new( scenes::list::ListScene::new( device_map ) ) );
    }

    {
      let scoped_device = device::ScopedDevice::new( state.device_id.clone(), state.device_map.clone() );
      builder.add_scene( scenes::device::ID, Box::new( scenes::device::DeviceScene::new( scoped_device ) ) );
    }

    {
      let scoped_device = device::ScopedDevice::new( state.device_id.clone(), state.device_map.clone() );
      builder.add_scene( scenes::connect::ID, Box::new( scenes::connect::ConnectScene::new( scoped_device ) ) );
    }

    Self{ builder }
  }

  pub fn run( mut self, action_tx: tokio::sync::mpsc::Sender<Action>, mut terminal: DefaultTerminal, mut event_rx: Receiver<scene::Event> ) {
    let mut scenes = self.builder.build( action_tx );

    while let Some( event ) = event_rx.blocking_recv() {
      match event {
        scene::Event::Key( key ) => {
          if ! scenes.key_down( key ) {
            match key.code {
              KeyCode::Char( 'q' ) | KeyCode::Esc => { return; },
              _ => { }
            }
          }
        },
        scene::Event::Tick( _time ) => {
          let _ = scenes.update();

          let _ = terminal.draw(| frame |{
            let main_layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
            let [ body_area, footer_area ] = frame.area().layout( &main_layout );

            // Main > Body
            scenes.draw( body_area, frame.buffer_mut() );

            // Main > Footer
            Paragraph::new( "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." )
              .centered()
              .render( footer_area, frame.buffer_mut() );
          });
        }
        scene::Event::Scene( event ) => {
          scenes.on_event( event )
        }
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

impl scene::Actionable for State {
  type Action = Action;

  async fn invoke( &mut self, action: Action ) -> scene::SceneEvent {
    match action {
      Action::Connect( _port ) => {
        let device_id = self.device_id.lock().unwrap().unwrap();

        if let Some( device ) = self.device_map.lock().await.get_mut( device_id ) {
          let _ = device.connect().await;
          return scene::SceneEvent::None;
        }

        return scene::SceneEvent::Pop;
      },
      Action::SelectDevice( id ) => {
        if let Ok( mut guard ) = self.device_id.lock() {
          *guard = Some( id );
          return scene::SceneEvent::Switch( scenes::device::ID )
        }
      },
      Action::DeselectDevice => {
        if let Ok( mut guard ) = self.device_id.lock() {
          *guard = None;
          return scene::SceneEvent::Switch( scenes::list::ID );
        }
      },
    }

    scene::SceneEvent::None
  }
}