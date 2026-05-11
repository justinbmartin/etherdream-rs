//! CLI tool to discover, connect and test Etherdream DAC's.
mod executors;

use std::net::SocketAddr;

use crossterm::event::{ self, Event, KeyCode, KeyEventKind };
use ratatui::prelude::*;
use ratatui::style::palette::tailwind::SLATE;
use ratatui::widgets::{ Block, List, ListItem, ListState, Paragraph };

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
        println!( "(discovery error: {:?})", err );
        return;
      }
    };

  ratatui::run(| terminal |{
    loop {
      if app.should_exit { break; }

      // Persist any discovered devices
      while let Ok( device_info ) = device_info_rx.try_recv() {
        app.devices.push( device_info.info().clone() );
      }

      // Render interface
      let _ = terminal.draw(| frame |{ app.render( frame ); });

      // Handle Inputs
      if let Ok( Event::Key( key ) ) = event::read() {
        if key.kind == KeyEventKind::Press {
          match key.code {
            KeyCode::Char( 'q' ) | KeyCode::Esc => app.on_escape(),
            KeyCode::Down => app.on_down(),
            KeyCode::Up => app.on_up(),
            KeyCode::Enter => app.on_enter(),
            _ => {}
          }
        }
      }
    }
  });
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

struct App {
  devices: Vec<etherdream::DeviceInfo>,
  devices_state: ListState,
  device_selected: Option<usize>,
  should_exit: bool
}

impl Default for App {
  fn default() -> Self {
    let mut state = ListState::default();
    state.select( Some( 0 ) );

    Self{
      devices: vec![
        etherdream::DeviceInfo::new( SocketAddr::from(( [10, 0, 0, 1], 6543 )), etherdream::protocol::Intrinsics::default() ),
        etherdream::DeviceInfo::new( SocketAddr::from(( [10, 0, 0, 2], 6543 )), etherdream::protocol::Intrinsics::default() )
      ],
      device_selected: None,
      devices_state: state,
      should_exit: false
    }
  }
}

impl App {
  fn render( &mut self, frame: &mut Frame ) {
    let main_layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
    let [ content_area, footer_area ] = frame.area().layout( &main_layout );

    if let Some( index ) = self.device_selected {
      // Main > Device
      let device_info = self.devices.get( index ).unwrap();
      let block = Block::bordered().title( Line::raw( format!( " Device: {}", device_info.address() ) ).centered() );

      Paragraph::new( format!( "MAC Address: {}", device_info.mac_address() ) )
        .centered()
        .block( block )
        .render( content_area, frame.buffer_mut() )

    } else {
      // Main > Devices
      let devices_list = DeviceList{ device_infos: &self.devices };
      frame.render_stateful_widget( devices_list, content_area, &mut self.devices_state );
    }

    // Main > Footer
    Paragraph::new( "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." )
      .centered()
      .render( footer_area, frame.buffer_mut() );
  }

  fn on_down( &mut self ) {
    let i = match self.devices_state.selected() {
      Some(i) => i.saturating_add( 1 ) % self.devices.len(),
      None => 0
    };

    self.devices_state.select( Some( i ) );
  }

  fn on_up( &mut self ) {
    let i = match self.devices_state.selected() {
      Some(i) => i.saturating_sub( 1 ) % self.devices.len(),
      None => 0
    };

    self.devices_state.select( Some( i ) );
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

struct DeviceList<'a> {
  device_infos: &'a Vec<etherdream::DeviceInfo>
}

impl<'a> StatefulWidget for DeviceList<'a> {
  type State = ListState;

  fn render( self, area: Rect, buf: &mut Buffer, state: &mut Self::State ) {
    let block = Block::bordered().title( Line::raw( " Devices " ).centered() );

    if self.device_infos.is_empty() {
      Paragraph::new( "(no devices)" )
        .centered()
        .block( block )
        .render( area, buf )
    } else {
      // Iterate through all `devices` and stylize them.
      let devices: Vec<ListItem> = self.device_infos
        .iter()
        .enumerate()
        .map(|(_i, device_info)| { ListItem::new( device_info.address().to_string() ) })
        .collect();

      // Create a List from all `devices` and highlight the currently selected one
      let devices_list = List::new( devices )
        .block( block )
        .highlight_style( Style::new().bg( SLATE.c800 ).add_modifier( Modifier::BOLD ) )
        .highlight_symbol( "> " )
        .highlight_spacing( ratatui::widgets::HighlightSpacing::Always );

      // We need to disambiguate this trait method as both `Widget` and `StatefulWidget` share the
      // same method name `render`.
      StatefulWidget::render( devices_list, area, buf, state );
    }
  }
}