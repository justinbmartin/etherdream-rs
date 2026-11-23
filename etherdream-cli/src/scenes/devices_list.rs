use std::fmt::Write;
use std::net::IpAddr;

use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::widgets::{ Row, Widget };

use crate::app;
use crate::scene;
use super::common;

pub struct DevicesListScene {
  devices_title: String,
  table: common::TableState<IpAddr,Device>,
  version: usize
}

impl DevicesListScene {
  pub fn new() -> Self {
    Self{
      devices_title: String::new(),
      table: common::TableState::new(),
      version: 0
    }
  }
}

impl scene::Scene<app::State> for DevicesListScene {
  fn on_key_down( &mut self, key: KeyEvent, ctx: &mut scene::Context<app::State> ) -> bool {
    match key.code {
      KeyCode::Enter => {
        if let Some( ip ) = self.table.selected() {
          ctx.invoke( app::Action::SelectDevice( *ip ) );
        }

        true
      }
      key => {
        self.table.handle_key_event( &key )
      }
    }
  }

  fn on_update( &mut self, ctx: &mut scene::Context<app::State> ) {
    let state = ctx.read_state();

    if state.devices_version() != self.version {
      self.table.clear();

      for ( ip, device ) in state.devices() {
        self.table.insert( *ip, Device{ ip_addr: ip.to_string(), mac_addr: device.mac_address().to_string() } );
      }

      self.version = state.devices_version();

      let _ = write!( &mut self.devices_title, " Etherdream Devices [{}] ", self.table.len() );
    }
  }

  fn on_draw( &mut self, area: Rect, buf: &mut Buffer, ctx: &scene::Context<app::State> ) {
    let layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
    let [ body_area, footer_area ] = area.layout( &layout );

    let block = common::make_block( Some( &self.devices_title ) );
    let inner_area = block.inner( body_area );

    if self.table.is_empty() {
      Widget::render( block, body_area, buf );
      common::draw_centered_line( "Listening for devices...", common::INFO_TEXT_STYLE.italic(), inner_area, buf );
      common::draw_footer( "Use 'q' to quit.", footer_area, buf );
    } else {
      Widget::render( block, body_area, buf );
      self.draw_list( inner_area, buf, ctx );
      common::draw_footer( "Use ↓↑ to move, '<Enter>' to select a device, 'q' to quit.", footer_area, buf );
    }
  }
}

impl DevicesListScene {
  fn draw_list( &mut self, area: Rect, buf: &mut Buffer, ctx: &scene::Context<app::State> ) {
    //let selected_ip = self.table.selected().unwrap();
    let state = ctx.read_state();

    const CONSTRAINTS: [Constraint; 3] = [
      Constraint::Length( 20 ),
      Constraint::Length( 35 ),
      Constraint::Fill( 1 ) ];

    self.table.render( CONSTRAINTS, area, buf, |( k, v ), is_selected |{
      let style = if is_selected { common::HIGHLIGHT_ROW_SELECTED_STYLE } else { common::HIGHLIGHT_TEXT_STYLE };

      let status = match state.sequencer( &k ) {
        Some( sequencer ) => if sequencer.is_playing() { "PLAYING" } else { "CONNECTED" },
        None => "DISCONNECTED"
      };

      Row::new([ v.ip_addr.as_str(), v.mac_addr.as_str(), status ]).style( style )
    });
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device Row

#[derive( Default )]
struct Device {
  ip_addr: String,
  mac_addr: String
}