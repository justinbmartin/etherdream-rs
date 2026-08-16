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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Action

pub enum Action {
  Connect( u16 ),
  SelectDevice( usize ),
  DeselectDevice
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

pub struct App {
  device_map: Arc<Mutex<device::DeviceMap>>,
  scenes: scene::Controller<MainScene>
}

impl App {
  pub fn new() -> Self {
    let device_id = Arc::new( Mutex::new( None::<usize> ) );
    let device_map = Arc::new( Mutex::new( device::DeviceMap::default() ) );

    // Scenes
    let main_scene = MainScene{ device_id: device_id.clone(), device_map: device_map.clone() };
    let mut builder = scene::Builder::new( main_scene );

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

    Self{
      device_map,
      scenes: builder.build()
    }
  }

  pub async fn run( &mut self, mut terminal: DefaultTerminal, mut discovery_rx: Receiver<etherdream::DiscoveredDeviceInfo> ) {
    let mut is_running = true;

    let ( events_controller, mut events_rx ) = event::EventController::start().await;

    while is_running {

      // Persist any discovered devices from the Etherdream discovery service
      while let Ok( device_info ) = discovery_rx.try_recv() {
        self.device_map.lock().unwrap().insert( device_info.info().clone() );
      }

      if let Some( event ) = events_rx.recv().await {
        match event {
          event::Event::KeyEvent( key ) => {
            if ! self.scenes.key_down( key ) {
              match key.code {
                KeyCode::Char( 'q' ) | KeyCode::Esc => { is_running = false; },
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

pub struct MainScene {
  device_map: Arc<Mutex<device::DeviceMap>>,
  device_id: Arc<Mutex<Option<usize>>>
}

impl scene::Actionable for MainScene {
  type Action = Action;

  fn invoke( &mut self, action: Action ) -> scene::Event {
    match action {
      Action::Connect( _port ) => {
        if let Ok( guard ) = self.device_id.lock() && let Some( device_id ) = *guard {
          if let Ok( mut guard ) = self.device_map.lock() && let Some( _device ) = guard.get_mut( device_id ) {
            //tokio::spawn( device.connect() );
          }
        }

        return scene::Event::Pop;
      },
      Action::SelectDevice( id ) => {
        if let Ok( mut guard ) = self.device_id.lock() {
          *guard = Some( id );
          return scene::Event::Switch( scenes::device::ID )
        }
      },
      Action::DeselectDevice => {
        if let Ok( mut guard ) = self.device_id.lock() {
          *guard = None;
          return scene::Event::Switch( scenes::list::ID );
        }
      },
    }

    scene::Event::None
  }
}