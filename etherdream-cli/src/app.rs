use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::{ DefaultTerminal, Frame };
use ratatui::layout::{ Constraint, Layout };
use ratatui::widgets::{ Paragraph, Widget };
use tokio::sync::mpsc::Receiver;

use crate::actions;
use crate::device::DeviceMap;
use crate::event;
use crate::scene;
use crate::scenes;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Events

pub enum Event {
  Connect( usize )
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

pub struct App {
  device_map: Rc<RefCell<DeviceMap>>,
  device_selected_id: Rc<RefCell<Option<usize>>>,
  is_running: bool,
  scenes: scene::Controller<scenes::SceneContext,Event>
}

impl App {
  pub fn new() -> Self {
    let device_map = Rc::new( RefCell::new( DeviceMap::default() ) );
    let device_selected_id = Rc::new( RefCell::new( None::<usize> ) );

    // Scenes
    let mut builder = scene::Builder::<scenes::SceneContext,Event>::new();

    {
      let device_map = device_map.clone();
      let device_selected_id = device_selected_id.clone();
      let assign_current_device = actions::AssignCurrentDevice::new( device_map, device_selected_id );

      builder.add_scene( "list", Box::new( scenes::list::ListScene::new( assign_current_device ) ) );
    }

    Self{
      device_map,
      device_selected_id,
      is_running: false,
      scenes: builder.build()
    }
  }

  pub async fn run( &mut self, mut terminal: DefaultTerminal, mut discovery_rx: Receiver<etherdream::DiscoveredDeviceInfo> ) {
    self.is_running = true;

    let mut ctx = scenes::SceneContext::new( self.device_map.clone(), self.device_selected_id.clone() );

    let ( events, mut events_rx ) = event::EventHandler::new();
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
        Some( event::Event::KeyEvent( key ) ) => self.on_key_event( &mut ctx, key ).await,
        Some( event::Event::Tick ) => (),
        _ => ()
      }
    }
  }

  async fn on_key_event( &mut self, ctx: &mut scenes::SceneContext, key: KeyEvent ) {
    match self.scenes.key_down( ctx, key ) {
      scene::Event::NotHandled => {
        match key.code {
          KeyCode::Char( 'q' ) | KeyCode::Esc => { self.is_running = false; },
          _ => {}
        }
      },
      scene::Event::Handled => { /* no-op */ },
      scene::Event::Custom( Event::Connect( _device_id ) ) => { /* no-op */ }
    };
  }

  fn render( &mut self, ctx: &scenes::SceneContext, frame: &mut Frame ) {
    let main_layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
    let [ body_area, footer_area ] = frame.area().layout( &main_layout );

    // Main > Body
    self.scenes.render( ctx, body_area, frame.buffer_mut() );

    // Main > Footer
    Paragraph::new( "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." )
      .centered()
      .render( footer_area, frame.buffer_mut() );
  }
}