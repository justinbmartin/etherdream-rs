use std::sync::{ Arc, Mutex };

use crossterm::event::KeyCode;
use ratatui::{ DefaultTerminal, Frame };
use ratatui::layout::{ Constraint, Layout };
use ratatui::widgets::{ Paragraph, Widget };
use tokio::sync::mpsc::Receiver;

use crate::device;
use crate::event;
use crate::scene;
use crate::scenes;

const FPS: f32 = 30.0;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Action

pub enum Action {
  Connect( u16 ),
  SelectDevice( usize ),
  DeselectDevice
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

pub struct App {
  is_running: bool,
  scenes: SceneController
}

impl App {
  pub fn new() -> Self {
    let device_id = Arc::new( Mutex::new( None::<usize> ) );
    let device_map = Arc::new( Mutex::new( device::DeviceMap::default() ) );

    // Scenes
    let mut builder = scene::Builder::<Self>::new();

    {
      let device_map = device::ReadOnlyDeviceMap::new( device_map.clone() );
      builder.add_scene( "list", Box::new( scenes::list::ListScene::new( device_map ) ) )
    };

    {
      let scoped_device = device::ScopedDevice::new( device_id.clone(), device_map.clone() );
      builder.add_scene( "device", Box::new( scenes::device::DeviceScene::new( scoped_device ) ) )
    };

    {
      let scoped_device = device::ScopedDevice::new( device_id.clone(), device_map.clone() );
      builder.add_scene( "connect", Box::new( scenes::connect::ConnectScene::new( scoped_device ) ) )
    };

    Self{
      is_running: false,
      scenes: builder.build(  )
    }
  }

  pub async fn run( &mut self, mut terminal: DefaultTerminal, mut discovery_rx: Receiver<etherdream::DiscoveredDeviceInfo> ) {
    let ( events_controller, mut events_rx ) = event::EventController::start().await;

    //
    self.is_running = true;
    while self.is_running {

      // Persist any discovered devices from the Etherdream discovery service
      while let Ok( device_info ) = discovery_rx.try_recv() {
        self.device_map.lock().unwrap().insert( device_info.info().clone() );
      }

      if let Some( event ) = events_rx.recv().await {
        match event {
          event::Event::KeyEvent( key ) => {
            if ! self.scenes.key_down( key ) {
              match key.code {
                KeyCode::Char( 'q' ) | KeyCode::Esc => { self.is_running = false; },
                _ => {}
              }
            }
          },
          event::Event::Tick( _time ) => {
            let _ = self.scenes.update();
            let _ = terminal.draw(| frame |{ self.render( frame ) });
          }
        }
      }
    }

    events_controller.stop().await;
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

pub struct SceneController {
  device_id: Arc<Mutex<Option<usize>>>,
  device_map: Arc<Mutex<device::DeviceMap>>
}

impl scene::ActionHandler for SceneController {
  type Event = Action;

  fn invoke(&mut self, action: Self::Event) -> scene::Event {
    match action {
      Action::Connect( _port ) => {
        return scene::Event::Pop;
      },
      Action::SelectDevice( id ) => {
        if let Ok( mut guard ) = self.device_id.lock() {
          *guard = Some( id );
          return scene::Event::Switch( "device" )
        }
      },
      Action::DeselectDevice => {
        if let Ok( mut guard ) = self.device_id.lock() {
          *guard = None;
          return scene::Event::Switch( "list" );
        }
      },
    }

    scene::Event::None
  }
}