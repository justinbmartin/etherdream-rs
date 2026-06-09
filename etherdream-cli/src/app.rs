use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::{ KeyCode, KeyEvent, KeyEventKind };
use ratatui::{ DefaultTerminal, Frame };
use ratatui::layout::{ Constraint, Layout };
use ratatui::widgets::{ Paragraph, Widget };
use tokio::sync::mpsc::Receiver;

use crate::device::DeviceMap;
use crate::event::{ Event, EventHandler };
use crate::executors;
use crate::scene::{ SceneContext, SceneEvent, SceneManager, SceneManagerBuilder };
use crate::scenes::{ self, SceneKey };

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

pub struct App {
  device_map: Rc<RefCell<DeviceMap>>,
  device_selected_id: Rc<RefCell<Option<usize>>>,
  is_running: bool,
  scenes: SceneManager<SceneKey>
}

impl App {
  pub fn new() -> Self {
    let device_map = Rc::new( RefCell::new( DeviceMap::default() ) );
    let device_selected_id = Rc::new( RefCell::new( None::<usize> ) );

    let scenes = SceneManagerBuilder::<SceneKey>::new( SceneKey::List, Box::new( scenes::ListScene::default() ) )
      .add_scene( SceneKey::Device, Box::new( scenes::DeviceScene::default() ) )
      .build();

    Self{
      device_map,
      device_selected_id,
      is_running: false,
      scenes
    }
  }

  pub async fn run( &mut self, mut terminal: DefaultTerminal, mut discovery_rx: Receiver<etherdream::DiscoveredDeviceInfo> ) {
    self.is_running = true;

    let mut ctx = SceneContext::new( self.device_map.clone(), self.device_selected_id.clone() );

    let ( events, mut events_rx ) = EventHandler::new();
    tokio::spawn( async move{ events.run().await } );

    while self.is_running {
      // Persist any discovered devices from the Etherdream discovery service
      while let Ok( device_info ) = discovery_rx.try_recv() {
        self.device_map.borrow_mut().insert( device_info.info().clone() );
      }

      // Render the terminal
      let _ = terminal.draw(| frame |{ self.render( &ctx, frame ) });

      // Handle any events
      match events_rx.recv().await {
        Some( Event::KeyEvent( key ) ) => self.on_key_event( &mut ctx, key ).await,
        Some( Event::Tick ) => (),
        _ => ()
      }
    }
  }

  async fn on_key_event( &mut self, ctx: &mut SceneContext, key: KeyEvent ) {
    if key.kind == KeyEventKind::Press {
      match self.scenes.current_scene().on_key_down( ctx, key ) {
        SceneEvent::Connect( id ) => {
          if let Some( device ) = self.device_map.borrow_mut().get_mut( id ) {
            let _ = device.connect().await;
          }
        }
        SceneEvent::Disconnect( id ) => {
          if let Some( device ) = self.device_map.borrow_mut().get_mut( id ) {
            let _ = device.disconnect().await;
          }
        }
        SceneEvent::Play( id ) => {
          if let Some( device ) = self.device_map.borrow_mut().get_mut( id ) {
            device.generate( Box::new( executors::Demo::new() ) ).await;
          }
        }
        SceneEvent::Select( id ) => {
          *self.device_selected_id.borrow_mut() = Some( id );
          self.scenes.set_scene( SceneKey::Device )
        }
        SceneEvent::Exit => {
          *self.device_selected_id.borrow_mut() = None;
          self.scenes.set_scene( SceneKey::List )
        }
        SceneEvent::NotHandled => {
          match key.code {
            KeyCode::Char( 'q' ) | KeyCode::Esc => {
              self.is_running = false;
            },
            _ => {}
          }
        }
        SceneEvent::Handled => { /* no-op */ }
      };
    }
  }

  fn render( &mut self, ctx: &SceneContext, frame: &mut Frame ) {
    let main_layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
    let [ content_area, footer_area ] = frame.area().layout( &main_layout );

    self.scenes.current_scene().render( ctx, content_area, frame.buffer_mut() );

    // Main > Footer
    Paragraph::new( "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." )
      .centered()
      .render( footer_area, frame.buffer_mut() );
  }
}