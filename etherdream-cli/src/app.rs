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
  device_map: Arc<Mutex<device::DeviceMap>>,
  device_selected_id: Arc<Mutex<Option<usize>>>,
  is_running: bool,
  scenes: scene::Controller<Event>
}

impl App {
  pub fn new() -> Self {
    let device_map = Arc::new( Mutex::new( device::DeviceMap::default() ) );
    let device_selected_id = Arc::new( Mutex::new( None::<usize> ) );

    //let shared_data = scenes::SharedData::new( device_map.clone(), device_selected_id.clone() );

    // Scenes
    let mut builder = scene::Builder::<Event>::new();
    builder.add_scene( scenes::list::make_list_scene_definition( device_map.clone(), device_selected_id.clone() ) );
    //builder.add_scene( scenes::device::make_connect_scene_definition( device_map.clone() ) );

    Self{
      device_map,
      device_selected_id,
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