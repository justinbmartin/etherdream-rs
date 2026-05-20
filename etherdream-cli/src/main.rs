//! CLI tool to discover, connect and test Etherdream DAC's.
mod executors;

use std::net::SocketAddr;
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
        app.devices.push( device_info.info().clone() );
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

enum Scenes { List, Info }

trait Scene {
  // ...
  fn on_key_press( &mut self, key: KeyCode ) -> bool;

  // ...
  fn render( &mut self, area: Rect, buf: &mut Buffer );
}

struct App<'a> {
  devices: Vec<etherdream::DeviceInfo>,
  device_selected: Option<usize>,
  scene: Scenes,
  scene_list: ListScene<'a>,
  scene_info: InfoScene<'a>,
  should_exit: bool
}

impl Default for App<'_> {
  fn default() -> Self {
    let device_infos = vec![
      etherdream::DeviceInfo::new( SocketAddr::from(( [10, 0, 0, 1], 6543 )), etherdream::protocol::Intrinsics::default() ),
      etherdream::DeviceInfo::new( SocketAddr::from(( [10, 0, 0, 2], 6543 )), etherdream::protocol::Intrinsics::default() )
    ];

    let list_scene = ListScene::new( &device_infos );
    let info_scene = InfoScene{};

    Self{
      devices: device_infos,
      device_selected: None,
      scene: Scenes::List,
      scene_info: info_scene,
      scene_list: list_scene,
      should_exit: false
    }
  }
}

impl<'a> App<'a> {
  fn on_key_event( &mut self, key: KeyEvent ) {
    if key.kind == KeyEventKind::Press {
      let handled =
        match self.scene {
          Scenes::Info => self.scene_info.on_key_press( key.code ),
          Scenes::List => self.scene_list.on_key_press( key.code )
        };

      if ! handled {
        /* handle scene changes */
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

  fn on_enter( &mut self ) {
    if self.device_selected.is_none() {
      self.device_selected = Some( self.devices_state.selected().unwrap() )
    }
  }

  fn on_escape( &mut self ) {
    if self.device_selected.is_some() {
      self.device_selected = None;
    } else {
      self.should_exit = true;
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene: List

struct ListScene<'a> {
  device_infos: &'a Vec<etherdream::DeviceInfo>,
  state: TableState
}

impl<'a> ListScene<'a> {
  fn new( device_infos: &'a Vec<etherdream::DeviceInfo> ) -> Self {
    let mut state = TableState::default();
    state.select( Some( 0 ) );

    Self{ device_infos, state }
  }
}

impl<'a> Scene for ListScene<'a> {
  fn on_key_press( &mut self, key: KeyCode ) -> bool {
    match key {
      KeyCode::Down => {
        let i =
          if let Some( i ) = self.state.selected() {
            i.saturating_add( 1 ) % self.device_infos.len()
          } else {
            0
          };

        self.state.select( Some( i ) );
        return true;
      },
      KeyCode::Up => {
        let i =
          if let Some( i ) = self.state.selected() {
            i.saturating_sub( 1 ) % self.device_infos.len()
          } else {
            0
          };

        self.state.select( Some( i ) );
        return true;
      },
      _ => {}
    }

    // Bubble-up all other key events
    false
  }

  fn render( &mut self, area: Rect, buf: &mut Buffer ) {
    let block = Block::bordered().title( Line::raw( " Etherdream Devices " ).centered() );

    if self.device_infos.is_empty() {
      Paragraph::new( "(no devices)" )
        .centered()
        .block( block )
        .render( area, buf )
    } else {
      let devices: Vec<Row> = self.device_infos
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

struct InfoScene<'a> {
  device_info: &'a etherdream::DeviceInfo
}

impl<'a> Scene for InfoScene<'a> {
  fn on_key_press( &mut self, key: KeyCode ) -> bool {
    // Bubble-up all other key events
    false
  }

  fn render( &mut self, area: Rect, buf: &mut Buffer ) {
    let block = Block::bordered().title( Line::raw( format!( " Device: {} ", self.device_info.address() ) ).centered() );

    Paragraph::new( format!( "MAC Address: {}", self.device_info.mac_address() ) )
      .centered()
      .block( block )
      .render( area, buf )
  }
}