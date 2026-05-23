use std::cell::RefCell;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::rc::Rc;
use std::time::Duration;

use crossterm::event::{ self, Event, KeyCode, KeyEvent, KeyEventKind };
use ratatui::prelude::*;
use ratatui::style::palette::tailwind::SLATE;
use ratatui::widgets::{ Block, Paragraph, Row, Table, TableState };
use tokio::sync::mpsc::Receiver;

use crate::device::DeviceMap;
use crate::scene::{ IsScene, Scene, SceneData, SceneEvent };

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

pub struct App {
  current_scene: Scene,
  device_map: Rc<RefCell<DeviceMap>>,
  device_selected_id: Rc<RefCell<Option<SocketAddr>>>,
  discovery_rx: Receiver<etherdream::DiscoveredDeviceInfo>,
  scenes: HashMap<Scene,Box<dyn IsScene>>,
  should_exit: bool
}

impl App {
  pub fn new( discovery_rx: Receiver<etherdream::DiscoveredDeviceInfo> ) -> Self {
    let device_map = Rc::new( RefCell::new( DeviceMap::default() ) );
    let device_selected_id = Rc::new( RefCell::new( None::<SocketAddr> ) );

    // TODO: Maybe wrap in `test` configuration option?
    let device_info = etherdream::DeviceInfo::new( SocketAddr::from(( [10, 0, 0, 1], 6543 )), etherdream::protocol::Intrinsics::default() );
    device_map.borrow_mut().insert( device_info );

    let device_info = etherdream::DeviceInfo::new( SocketAddr::from(( [10, 0, 0, 2], 6543 )), etherdream::protocol::Intrinsics::default() );
    device_map.borrow_mut().insert( device_info );

    let scene_data = SceneData::new( device_map.clone(), device_selected_id.clone() );

    let mut scenes: HashMap<Scene,Box<dyn IsScene>> = HashMap::new();
    scenes.insert( Scene::Info, Box::new( InfoScene::new( scene_data.clone() ) ) );
    scenes.insert( Scene::List, Box::new( ListScene::new( scene_data ) ) );

    Self{
      current_scene: Scene::List,
      device_map,
      device_selected_id,
      discovery_rx,
      scenes,
      should_exit: false
    }
  }

  pub fn run( &mut self ) {
    ratatui::run(| terminal |{
      loop {
        if self.should_exit { break; }

        // Capture any discovered devices from the Etherdream discovery service
        while let Ok( device_info ) = self.discovery_rx.try_recv() {
          self.device_map.borrow_mut().insert( device_info.info().clone() );
        }

        // Render
        let _ = terminal.draw(| frame |{ self.render( frame ); });

        // Handle user-input
        if let Ok( true ) = event::poll( Duration::from_millis( 100 ) ) {
          if let Ok( Event::Key( key ) ) = event::read() {
            self.on_key_event( key );
          }
        }
      }
    });
  }

  fn on_key_event( &mut self, key: KeyEvent ) {
    if key.kind == KeyEventKind::Press {
      let handled =
        if let Some( scene ) = self.scenes.get_mut( &self.current_scene ) {
          scene.on_key_press( key.code )
        } else {
          SceneEvent::NotHandled
        };

      match handled {
        SceneEvent::Select( address ) => {
          *self.device_selected_id.borrow_mut() = Some( address );
          self.current_scene = Scene::Info;
        },
        SceneEvent::Exit => {
          *self.device_selected_id.borrow_mut() = None;
          self.current_scene = Scene::List;
        },
        SceneEvent::NotHandled => {
          match key.code {
            KeyCode::Char( 'q' ) | KeyCode::Esc => {
              self.should_exit = true;
            },
            _ => {}
          }
        }
        SceneEvent::Handled => { /* no-op */ }
      };
    }
  }

  fn render( &mut self, frame: &mut Frame ) {
    let main_layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
    let [ content_area, footer_area ] = frame.area().layout( &main_layout );

    if let Some( scene ) = self.scenes.get_mut( &self.current_scene ) {
      scene.render( content_area, frame.buffer_mut() )
    }

    // Main > Footer
    Paragraph::new( "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." )
      .centered()
      .render( footer_area, frame.buffer_mut() );
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene: List

struct ListScene {
  data: SceneData,
  device_map_version: usize,
  sorted_device_keys: Vec<SocketAddr>, // Scene cache of sorted device keys
  state: TableState
}

impl ListScene {
  fn new( data: SceneData ) -> Self {
    Self{
      data,
      device_map_version: 0,
      sorted_device_keys: Vec::new(),
      state: TableState::new().with_selected( Some( 0 ) )
    }
  }
}

impl IsScene for ListScene {
  fn on_key_press( &mut self, key: KeyCode ) -> SceneEvent {
    match key {
      KeyCode::Down => {
        let i = self.state.selected().unwrap_or( 0 ).saturating_add( 1 ) % self.data.device_map().len();
        self.state.select( Some( i ) );
        SceneEvent::Handled
      },
      KeyCode::Up => {
        let i = self.state.selected().unwrap_or( 0 ).saturating_sub( 1 ) % self.data.device_map().len();
        self.state.select( Some( i ) );
        SceneEvent::Handled
      },
      KeyCode::Enter => {
        //let i = self.state.selected().unwrap_or( 0 );
        if let Some( index ) = self.state.selected() && let Some( addr ) = self.sorted_device_keys.get( index ) {
          SceneEvent::Select( *addr )
        } else {
          SceneEvent::NotHandled
        }
      }
      _ => {
        SceneEvent::NotHandled
      }
    }
  }

  fn render( &mut self, area: Rect, buf: &mut Buffer ) {
    let block = Block::bordered().title( Line::raw( " Etherdream Devices " ).centered() );

    // Refresh our local sorted device cache if the remote device map has changed
    if self.data.device_map().version() != self.device_map_version {
      self.sorted_device_keys = self.data.device_map()
        .iter()
        .map( |(&addr,_)|{ addr } )
        .collect();
      self.sorted_device_keys.sort();
    }

    if self.sorted_device_keys.is_empty() {
      Paragraph::new( "(no devices)" ).centered().block( block ).render( area, buf );
      return;
    }

    let rows: Vec<Row> = self.sorted_device_keys
      .iter()
      .filter_map(| addr |{
        if let Some( device ) = self.data.device_map().get( addr ) {
          Some( Row::new([
            addr.to_string(),
            device.info().mac_address().to_string(),
          ]) )
        } else {
          None
        }
      })
      .collect();

    let constraints = [ Constraint::Length( 25 ), Constraint::Fill( 1 ) ];

    let table = Table::new( rows, constraints )
      .block( block )
      .header( Row::new(vec![ "Address", "MAC" ]).style( Style::new().bold() ) )
      .highlight_spacing( ratatui::widgets::HighlightSpacing::Always )
      .highlight_symbol( "> " )
      .row_highlight_style( Style::new().bg( SLATE.c800 ).add_modifier( Modifier::BOLD ) );

    StatefulWidget::render( table, area, buf, &mut self.state );
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene: Info

struct InfoScene {
  data: SceneData
}

impl InfoScene {
  fn new( data: SceneData ) -> Self {
    Self{ data }
  }
}

impl IsScene for InfoScene {
  fn on_key_press( &mut self, key: KeyCode ) -> SceneEvent {
    match key {
      KeyCode::Esc | KeyCode::Char( 'q' ) => SceneEvent::Exit,
      _ => SceneEvent::NotHandled
    }
  }

  fn render( &mut self, area: Rect, buf: &mut Buffer ) {
    if let Some( device ) = self.data.selected_device() {
      let block = Block::bordered().title( Line::raw( format!( " Device: {} ", device.info().address() ) ).centered() );

      Paragraph::new( format!( "MAC Address: {}", device.info().mac_address() ) )
        .centered()
        .block( block )
        .render( area, buf )
    }
  }
}