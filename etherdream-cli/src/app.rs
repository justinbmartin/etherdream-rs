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
  Select( usize ),
  Deselect
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

pub struct App {
  device_id: Arc<Mutex<Option<usize>>>,
  device_map: Arc<Mutex<device::DeviceMap>>,
  is_running: bool,
  scenes: scene::Controller<Action>
}

impl App {
  pub fn new() -> Self {
    let device_id = Arc::new( Mutex::new( None::<usize> ) );
    let device_map = Arc::new( Mutex::new( device::DeviceMap::default() ) );

    // Scenes
    let mut builder = scene::Builder::<Action>::new();

    let scene_list_id = {
      let device_map = device::ReadOnlyDeviceMap::new( device_map.clone() );
      builder.add_scene( Box::new( scenes::list::ListScene::new( device_map ) ) )
    };

    let scene_device_id = {
      let scoped_device = device::ScopedDevice::new( device_id.clone(), device_map.clone() );
      builder.add_scene( Box::new( scenes::device::DeviceScene::new( scoped_device ) ) )
    };

    // Actions
    let action_fn = {
      let device_id = device_id.clone();

      Box::new( move | action |{
        match action {
          Action::Select( id ) => {
            if let Ok( mut guard ) = device_id.lock() {
              *guard = Some( id );
              Some( scene_device_id )
            } else {
              None
            }
          },
          Action::Deselect => {
            if let Ok( mut guard ) = device_id.lock() {
              *guard = None;
              Some( scene_list_id )
            } else {
              None
            }
          }
        }
      })
    };

    Self{
      device_id,
      device_map,
      is_running: false,
      scenes: builder.build( action_fn )
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