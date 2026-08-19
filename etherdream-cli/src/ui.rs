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

pub struct UI {
  scenes: scene::Controller<MainScene>
}

impl UI {
  pub fn new(
    action_tx: tokio::sync::mpsc::Sender<Action>,
    device_id: Arc<Mutex<Option<usize>>>,
    device_map: Arc<tokio::sync::Mutex<device::DeviceMap>>
  ) -> Self {
    
    // Scenes
    let mut builder = scene::Builder::new( action_tx );

    {
      let device_map = device::ReadOnlyDeviceMap::new( device_map.clone() );
      builder.add_scene( scenes::list::ID, Box::new( scenes::list::ListScene::new( device_map ) ) );
    }

    {
      let scoped_device = device::ScopedDevice::new( device_id.clone(), device_map.clone() );
      builder.add_scene( scenes::device::ID, Box::new( scenes::device::DeviceScene::new( scoped_device ) ) );
    }

    {
      let scoped_device = device::ScopedDevice::new( device_id.clone(), device_map.clone() );
      builder.add_scene( scenes::connect::ID, Box::new( scenes::connect::ConnectScene::new( scoped_device ) ) );
    }

    Self{ scenes: builder.build() }
  }

  pub fn run( mut self, mut terminal: DefaultTerminal, mut event_rx: Receiver<scene::Event> ) {
    while let Some( event ) = event_rx.blocking_recv() {
      match event {
        scene::Event::Key( key ) => {
          if ! self.scenes.key_down( key ) {
            match key.code {
              KeyCode::Char( 'q' ) | KeyCode::Esc => { return; },
              _ => { }
            }
          }
        },
        scene::Event::Tick( _time ) => {
          let _ = self.scenes.update();
          let _ = terminal.draw(| frame |{ self.render( frame ) });
        }
        scene::Event::Scene( event ) => {
          self.scenes.on_event( event )
        }
      }
    }
  }

  fn render( &mut self, frame: &mut Frame ) {
    let main_layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
    let [ body_area, footer_area ] = frame.area().layout( &main_layout );

    // Main > Body
    self.scenes.draw( body_area, frame.buffer_mut() );

    // Main > Footer
    Paragraph::new( "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." )
      .centered()
      .render( footer_area, frame.buffer_mut() );
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Scene Controller

pub struct MainScene {
  device_id: Arc<Mutex<Option<usize>>>,
  device_map: Arc<tokio::sync::Mutex<device::DeviceMap>>
}

impl MainScene {
  pub fn new( device_id: Arc<Mutex<Option<usize>>>, device_map: Arc<tokio::sync::Mutex<device::DeviceMap>> ) -> Self {
    Self{ device_id, device_map }
  }
}

impl scene::Actionable for MainScene {
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