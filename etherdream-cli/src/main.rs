//! CLI tool to discover, connect and test Etherdream DAC's.
mod executors;

use std::net::SocketAddr;
use std::time::Duration;

use crossterm::event::{ self, Event, KeyCode };
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

  // https://ratatui.rs/recipes/apps/terminal-and-event-handler/
  ratatui::run(| terminal |{
    loop {
      if should_quit() {
        break;
      }

      while let Ok( device_info ) = device_info_rx.try_recv() {
        app.devices.push( device_info.info().clone() );
      }

      let _ = terminal.draw(| frame |{ app.render( frame ); });

      // 3. Handle Inputs
      if let Ok( Event::Key( key ) ) = event::read() {
        match key.code {
          KeyCode::Char( 'q' ) | KeyCode::Esc => break,
          KeyCode::Down => app.on_down(),
          KeyCode::Up => app.on_up(),
          _ => {}
        }
      }

      if app.should_exit { break; }
    }
  });
}

fn should_quit() -> bool {
  if let Ok( true ) = event::poll( Duration::from_secs( 0 ) ) {
    if let Ok( event ) = event::read() {
      return event
        .as_key_press_event()
        .is_some_and(|key| key.code == KeyCode::Esc || key.code == KeyCode::Char( 'q' ) );
    }
  }

  false
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

struct App {
  devices: Vec<etherdream::DeviceInfo>,
  devices_state: ListState,
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
      devices_state: state,
      should_exit: false
    }
  }
}

impl App {
  fn render( &mut self, frame: &mut Frame ) {
    let main_layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
    let [ content_area, footer_area ] = frame.area().layout( &main_layout );

    // Main > Footer
    Paragraph::new( "Use ↓↑ to move, ← to unselect, → to change status, g/G to go top/bottom." )
      .centered()
      .render( footer_area, frame.buffer_mut() );

    // Main > Content
    let content_layout = Layout::horizontal([ Constraint::Length( 20 ), Constraint::Fill( 1 ) ]);
    let [ devices_area, info_area ] = content_area.layout( &content_layout );

    // Main > Content > Devices
    let devices_list = DeviceList{ device_infos: &self.devices };
    frame.render_stateful_widget( devices_list, devices_area, &mut self.devices_state );

    // Main > Content > Info
    frame.render_widget( Block::bordered().title( " Info " ), info_area );
  }

  // 2. Define methods to update state on keypress
  fn on_down( &mut self ) {
    let i = match self.devices_state.selected() {
      Some(i) => {
        if i >= self.devices.len() - 1 {
          0
        } else {
          i + 1
        }
      }
      None => 0,
    };

    self.devices_state.select( Some( i ) );
  }

  fn on_up( &mut self ) {
    let i = match self.devices_state.selected() {
      Some(i) => {
        if i == 0 {
          self.devices.len() - 1
        } else {
          i - 1
        }
      }
      None => 0,
    };

    self.devices_state.select( Some( i ) );
  }
}

struct DeviceList<'a> {
  device_infos: &'a Vec<etherdream::DeviceInfo>
}

impl<'a> StatefulWidget for DeviceList<'a> {
  type State = ListState;

  fn render( self, area: Rect, buf: &mut Buffer, state: &mut Self::State ) {
    let block = Block::bordered().title( Line::raw( " Devices " ) );

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