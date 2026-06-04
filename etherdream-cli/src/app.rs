use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crossterm::event::{ KeyCode, KeyEvent, KeyEventKind };
use ratatui::{ DefaultTerminal, Frame };
use ratatui::layout::{ Constraint, Layout };
use ratatui::widgets::{ Paragraph, Widget };
use tokio::sync::mpsc::Receiver;

use crate::device::DeviceMap;
use crate::event::{ Event, EventHandler };
use crate::executors;
use crate::scene::{ self, Context, IsScene, Scene, SceneEvent };

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

pub struct App {
  current_scene: Scene,
  device_map: Rc<RefCell<DeviceMap>>,
  device_selected_id: Rc<RefCell<Option<usize>>>,
  is_running: bool,
  scenes: HashMap<Scene,Box<dyn IsScene>>
}

impl App {
  pub fn new() -> Self {
    let device_map = Rc::new( RefCell::new( DeviceMap::default() ) );
    let device_selected_id = Rc::new( RefCell::new( None::<usize> ) );

    let mut scenes: HashMap<Scene,Box<dyn IsScene>> = HashMap::new();
    scenes.insert( Scene::Info, Box::new( scene::InfoScene::default() ) );
    scenes.insert( Scene::List, Box::new( scene::ListScene::default() ) );

    Self{
      current_scene: Scene::List,
      device_map,
      device_selected_id,
      is_running: false,
      scenes
    }
  }

  pub async fn run( &mut self, mut terminal: DefaultTerminal, mut discovery_rx: Receiver<etherdream::DiscoveredDeviceInfo> ) {
    self.is_running = true;

    let mut ctx = Context::new( self.device_map.clone(), self.device_selected_id.clone() );

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

  async fn on_key_event( &mut self, ctx: &mut Context, key: KeyEvent ) {
    if key.kind == KeyEventKind::Press {
      let handled =
        if let Some( scene ) = self.scenes.get_mut( &self.current_scene ) {
          scene.on_key_down( ctx, key.code )
        } else {
          SceneEvent::NotHandled
        };

      match handled {
        SceneEvent::Connect( id ) => {
          if let Some( device ) = self.device_map.borrow_mut().get_mut( id ) {
            match etherdream::connect( *device.info() ).await {
              Ok( client ) => {
                let generator = etherdream::make_generator( client, Box::new( executors::Noop::new() ) );
                device.set_generator( generator );
              },
              Err( _ ) => {
                // TODO: Error block...
                println!( "FAILED to connect..." )
              }
            }
          }
        }
        SceneEvent::Disconnect( id ) => {
          if let Some( device ) = self.device_map.borrow_mut().get_mut( id ) {
            device.disconnect()
          }
        }
        SceneEvent::Play( id ) => {
          if let Some( device ) = self.device_map.borrow_mut().get_mut( id ) {
            if let Some( generator ) = device.take_generator() {
              if let Ok( client ) = generator.into_client().await {
                let executor = Box::new( executors::Demo::new() );
                let mut generator = etherdream::make_generator( client, executor );
                generator.start().await;
                device.set_generator( generator );
              }
            }
          }
        }
        SceneEvent::Select( id ) => {
          *self.device_selected_id.borrow_mut() = Some( id );
          self.current_scene = Scene::Info;
        }
        SceneEvent::Exit => {
          *self.device_selected_id.borrow_mut() = None;
          self.current_scene = Scene::List;
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

  fn render( &mut self, ctx: &Context, frame: &mut Frame ) {
    let main_layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
    let [ content_area, footer_area ] = frame.area().layout( &main_layout );

    if let Some( scene ) = self.scenes.get_mut( &self.current_scene ) {
      scene.render( ctx, content_area, frame.buffer_mut() )
    }

    // Main > Footer
    Paragraph::new( "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." )
      .centered()
      .render( footer_area, frame.buffer_mut() );
  }
}