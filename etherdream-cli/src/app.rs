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
  Device( usize ),
  List
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

pub struct App {
  devices: Arc<Mutex<Devices>>,
  is_running: bool,
  scenes: scene::Controller<Action>,
  selected: Arc<Mutex<Option<usize>>>
}

pub struct Devices {
  pub devices: device::DeviceMap,
  pub selected_id: Option<usize>
}

impl App {
  pub fn new() -> Self {
    let device_map = Arc::new( Mutex::new( device::DeviceMap::default() ) );
    let devices = Arc::new( Mutex::new( Devices{ devices: device::DeviceMap::default(), selected_id: None } ) );
    let selected = Arc::new( Mutex::new( None::<usize> ) );

    // Scenes
    let mut builder = scene::Builder::<Action>::new();

    let scene_list_id = {
      let device_map = device::ReadOnlyDeviceMap::new( device_map.clone() );
      builder.add_scene( Box::new( scenes::list::ListScene::new( device_map ) ) )
    };

    let scene_device_id = {
      let device = device::ScopedDeviceMap::new( selected.clone(), device_map.clone() );
      builder.add_scene( Box::new( scenes::device::DeviceScene::new( device ) ) )
    };

    // Actions
    {
      let selected = selected.clone();

      builder.add_action( scene_list_id, Box::new( move | action |{
        match action {
          Action::Device( device_id ) => {
            *selected.lock().unwrap() = Some( device_id );
            scene_device_id
          },
          Action::List => {
            scene_list_id
          }
        }
      } ) );
    }

    Self{
      devices,
      is_running: false,
      scenes: builder.build(),
      selected
    }
  }

  pub async fn run( &mut self, mut terminal: DefaultTerminal, mut discovery_rx: Receiver<etherdream::DiscoveredDeviceInfo> ) {
    let ( events_controller, mut events_rx ) = event::EventController::start().await;

    //
    self.is_running = true;
    while self.is_running {

      // Persist any discovered devices from the Etherdream discovery service
      while let Ok( device_info ) = discovery_rx.try_recv() {
        println!("test");
        self.devices.lock().unwrap().devices.insert( device_info.info().clone() );
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