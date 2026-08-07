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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Events

pub enum Event {
  Connect( usize )
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

pub struct App {
  devices: Arc<Mutex<Devices>>,
  is_running: bool,
  scenes: scene::Controller<Event>
}

pub struct Devices {
  pub devices: device::DeviceMap,
  pub selected_id: Option<usize>
}

impl App {
  pub fn new() -> Self {
    let devices = Arc::new( Mutex::new( Devices{ devices: device::DeviceMap::default(), selected_id: None } ) );

    // Scenes
    let mut builder = scene::Builder::<Event>::new();
    builder.add_scene( "list", scenes::list::make_list_scene_definition( devices.clone() ) );
    builder.add_scene( "device", scenes::device::make_device_scene_definition( devices.clone() ) );

    Self{
      devices,
      is_running: false,
      scenes: builder.build()
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
            if ! self.scenes.key_down( key ).await {
              match key.code {
                KeyCode::Char( 'q' ) | KeyCode::Esc => { self.is_running = false; },
                _ => {}
              }
            }
          },
          event::Event::Tick( _time ) => {
            let _ = self.scenes.update().await;
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
    self.scenes.render( body_area, frame.buffer_mut() );

    // Main > Footer
    Paragraph::new( "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." )
      .centered()
      .render( footer_area, frame.buffer_mut() );
  }
}