//! CLI tool to discover, connect and test Etherdream DAC's.
mod executors;

use std::cell::RefCell;
use std::net::SocketAddr;
use std::rc::Rc;
use std::time::Duration;

use crossterm::event::{ self, Event, KeyCode, KeyEvent, KeyEventKind };
use ratatui::prelude::*;
use ratatui::style::palette::tailwind::SLATE;
use ratatui::widgets::{ Block, Paragraph, Row, Table, TableState };

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Main

#[tokio::main]
async fn main() {
  let mut app = App::default();

  // Start the Etherdream discovery service
  let ( device_info_tx, mut device_info_rx ) = tokio::sync::mpsc::channel( 16 );

  let _discovery_server =
    match etherdream::discover( device_info_tx ).await {
      Ok( server ) => server,
      Err( err ) => {
        eprintln!( "Failed to start Etherdream discovery: {:?}", err );
        return;
      }
    };

  ratatui::run(| terminal |{
    loop {
      if app.should_exit { break; }

      // Capture any discovered devices from the Etherdream discovery service
      while let Ok( device_info ) = device_info_rx.try_recv() {
        app.device_infos.borrow_mut().push( device_info.info().clone() );
      }

      // Render
      let _ = terminal.draw(| frame |{ app.render( frame ); });

      // Handle any user-input
      //
      // We poll here to ensure we do not block on event `read`. This ensures
      // that we always handle any discovered devices from `device_info_rx`.
      if let Ok( true ) = event::poll( Duration::from_secs( 0 ) ) {
        if let Ok( Event::Key( key ) ) = event::read() {
          app.on_key_event( key );
        }
      }
    }
  });
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

#[derive( PartialEq )]
enum Scenes { List, Info }

trait Scene {
  // ...
  fn on_key_press( &mut self, key: KeyCode ) -> bool;

  // ...
  fn render( &mut self, area: Rect, buf: &mut Buffer );
}

struct App {
  device_infos: Rc<RefCell<Vec<etherdream::DeviceInfo>>>,
  device_selected: Rc<RefCell<Option<usize>>>,
  scene: Scenes,
  scene_list: ListScene,
  scene_info: InfoScene,
  should_exit: bool
}

impl Default for App {
  fn default() -> Self {
    let device_infos = Rc::new( RefCell::new( vec![
      etherdream::DeviceInfo::new( SocketAddr::from(( [10, 0, 0, 1], 6543 )), etherdream::protocol::Intrinsics::default() ),
      etherdream::DeviceInfo::new( SocketAddr::from(( [10, 0, 0, 2], 6543 )), etherdream::protocol::Intrinsics::default() )
    ] ) );

    let device_selected = Rc::new( RefCell::new( None::<usize> ) );
    let scene_list = ListScene::new( device_infos.clone(), device_selected.clone() );
    let scene_info = InfoScene::new( device_infos.clone(), device_selected.clone() );

    Self{
      device_infos,
      device_selected,
      scene: Scenes::List,
      scene_info,
      scene_list,
      should_exit: false
    }
  }
}

impl App {
  fn on_key_event( &mut self, key: KeyEvent ) {
    if key.kind == KeyEventKind::Press {
      let handled =
        match self.scene {
          Scenes::Info => self.scene_info.on_key_press( key.code ),
          Scenes::List => self.scene_list.on_key_press( key.code )
        };

      if ! handled {
        match key.code {
          KeyCode::Char( 'q' ) | KeyCode::Esc => {
            if self.device_selected.borrow().is_some() {
              *self.device_selected.borrow_mut() = None;
              self.scene = Scenes::List;
            } else {
              self.should_exit = true;
            }
          },
          KeyCode::Enter => {
            if self.scene == Scenes::List && self.device_selected.borrow().is_some() {
              self.scene = Scenes::Info;
            }
          },
          _ => {}
        }
      }
    }
  }

  fn render( &mut self, frame: &mut Frame ) {
    let main_layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
    let [ content_area, footer_area ] = frame.area().layout( &main_layout );

    match self.scene {
      Scenes::Info => self.scene_info.render( content_area, frame.buffer_mut() ),
      Scenes::List => self.scene_list.render( content_area, frame.buffer_mut() ),
    }

    // Main > Footer
    Paragraph::new( "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." )
      .centered()
      .render( footer_area, frame.buffer_mut() );
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene: List

struct ListScene {
  device_infos: Rc<RefCell<Vec<etherdream::DeviceInfo>>>,
  device_selected: Rc<RefCell<Option<usize>>>,
  state: TableState
}

impl ListScene {
  fn new( device_infos: Rc<RefCell<Vec<etherdream::DeviceInfo>>>, device_selected: Rc<RefCell<Option<usize>>> ) -> Self {
    let mut state = TableState::default();
    state.select( Some( 0 ) );

    Self{ device_infos, device_selected, state }
  }
}

impl Scene for ListScene {
  fn on_key_press( &mut self, key: KeyCode ) -> bool {
    match key {
      KeyCode::Down => {
        let i =
          if let Some( i ) = self.state.selected() {
            i.saturating_add( 1 ) % self.device_infos.borrow().len()
          } else {
            0
          };

        self.state.select( Some( i ) );
        return true;
      },
      KeyCode::Up => {
        let i =
          if let Some( i ) = self.state.selected() {
            i.saturating_sub( 1 ) % self.device_infos.borrow().len()
          } else {
            0
          };

        self.state.select( Some( i ) );
        return true;
      },
      KeyCode::Enter => {
        if let Some( index ) = self.state.selected() {
          *self.device_selected.borrow_mut() = Some( index );
        }
      }
      _ => {}
    }

    // Bubble-up all other key events
    false
  }

  fn render( &mut self, area: Rect, buf: &mut Buffer ) {
    let block = Block::bordered().title( Line::raw( " Etherdream Devices " ).centered() );

    if self.device_infos.borrow().is_empty() {
      Paragraph::new( "(no devices)" )
        .centered()
        .block( block )
        .render( area, buf )
    } else {
      let devices: Vec<Row> = self.device_infos
        .borrow()
        .iter()
        .map(|di|{ Row::new([
          di.address().to_string(),
          di.mac_address().to_string(),
        ]) }).collect();

      let constraints = [ Constraint::Length( 25 ), Constraint::Fill( 1 ) ];

      let table = Table::new( devices, constraints )
        .block( block )
        .header( Row::new(vec![ "Address", "MAC" ]).style( Style::new().bold() ) )
        .row_highlight_style( Style::new().bg( SLATE.c800 ).add_modifier( Modifier::BOLD ) )
        .highlight_symbol( "> " )
        .highlight_spacing( ratatui::widgets::HighlightSpacing::Always );

      // We need to disambiguate this trait method as both `Widget` and `StatefulWidget` share the
      // same method name `render`.
      StatefulWidget::render( table, area, buf, &mut self.state );
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene: Info

struct InfoScene {
  device_infos: Rc<RefCell<Vec<etherdream::DeviceInfo>>>,
  device_selected: Rc<RefCell<Option<usize>>>
}

impl InfoScene {
  fn new( device_infos: Rc<RefCell<Vec<etherdream::DeviceInfo>>>, device_selected: Rc<RefCell<Option<usize>>> ) -> Self {
    Self{ device_infos, device_selected }
  }
}

impl Scene for InfoScene {
  fn on_key_press( &mut self, _key: KeyCode ) -> bool {
    // Bubble-up all other key events
    false
  }

  fn render( &mut self, area: Rect, buf: &mut Buffer ) {
    if let Some( device_index ) = *self.device_selected.borrow() &&
       let Some( device_info ) = self.device_infos.borrow().get( device_index ) {
      let block = Block::bordered().title( Line::raw( format!( " Device: {} ", device_info.address() ) ).centered() );

      Paragraph::new( format!( "MAC Address: {}", device_info.mac_address() ) )
        .centered()
        .block( block )
        .render( area, buf )
    }
  }
}