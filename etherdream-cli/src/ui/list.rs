use std::net::SocketAddr;

use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::widgets::{ Cell, Row, StatefulWidget, Table, TableState, Widget };

use crate::device::Device;
use crate::scene;
use crate::ui::{ Action, State };
use super::common;

pub struct ListScene {
  device_addrs: Vec<SocketAddr>,
  table: TableState,
  version: usize,
}

impl ListScene {
  pub fn new() -> Self {
    Self{
      device_addrs: Vec::new(),
      table: TableState::new(),
      version: 0
    }
  }
}

impl scene::Scene<State> for ListScene {
  fn on_key_down( &mut self, key: KeyEvent, ctx: &mut scene::Context<State> ) -> bool {
    match key.code {
      KeyCode::Down => {
        let next_idx = ( self.table.selected().unwrap_or( 0 ) + 1 ).checked_rem( self.device_addrs.len() );
        self.table.select( next_idx );
        true
      }
      KeyCode::Enter => {
        if let Some( addr ) = self.selected_addr() {
          ctx.invoke( Action::SelectDevice( *addr ) );
        }

        true
      }
      KeyCode::Up => {
        self.table.select_previous();
        true
      }
      _ => false
    }
  }

  fn on_update( &mut self, ctx: &mut scene::Context<State> ) {
    let state = ctx.state();
    let devices = state.devices();

    if devices.version() != self.version {
      let mut addrs: Vec<SocketAddr> = devices.iter().map(|( &d, _ )| d ).collect();
      let mut updated_idx = None::<usize>;

      if ! addrs.is_empty() {
        addrs.sort();

        if let Some( idx ) = self.table.selected() {
          if let Some( addr ) = self.device_addrs.get( idx ) && let Ok( idx ) = addrs.binary_search( addr ) {
            // The device index may have changed. Update the device index to reflect any new position.
            updated_idx = Some( idx );
          } else {
            // The previously selected device no longer exists.
            updated_idx = Some( idx.min( addrs.len() ) );
          }
        } else {
          // This is the first time the table has been populated.
          updated_idx = Some( 0 );
        }
      }

      self.device_addrs = addrs;
      self.table.select( updated_idx );
      self.version = devices.version();
    }
  }

  fn on_draw( &mut self, area: Rect, buf: &mut Buffer, ctx: &scene::Context<State> ) {
    let layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
    let [ body_area, footer_area ] = area.layout( &layout );

    let title = format!( " Etherdream Devices [{}] ", self.device_addrs.len() );
    let block = common::make_block( Some( &title ) );
    let inner_area = block.inner( body_area );

    if self.device_addrs.is_empty() {
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

impl ListScene {
  fn draw_list( &mut self, area: Rect, buf: &mut Buffer, ctx: &scene::Context<State> ) {
    let mut rows = Vec::with_capacity( self.device_addrs.len() );

    if let Some( selected_idx ) = self.table.selected() {
      let state = ctx.state();

      for ( i, addr ) in self.device_addrs.iter().enumerate() {
        if let Some( device ) = state.devices().get( addr ) {
          let style = if selected_idx == i { common::HIGHLIGHT_ROW_SELECTED_STYLE } else { common::HIGHLIGHT_TEXT_STYLE };

          rows.push(
            Row::new([
              Cell::new( device.info().ip().to_string() ),
              Cell::new( device.info().mac_address().to_string() ),
              Cell::new( device_status( &device ) )
            ]).style( style )
          );
        }
      }
    }

    const CONSTRAINTS: [Constraint; 3] = [
      Constraint::Length( 20 ),
      Constraint::Length( 35 ),
      Constraint::Fill( 1 ) ];

    let table = Table::new( rows, CONSTRAINTS )
      .header( Row::new([ "IP", "MAC", "STATUS" ]).style( common::TABLE_HEADER_STYLE ) );

    StatefulWidget::render( table, area, buf, &mut self.table );
  }

  fn selected_addr( &self ) -> Option<&SocketAddr> {
    self.table.selected().and_then(| i |{ self.device_addrs.get( i ) })
  }
}

fn device_status( device: &Device ) -> &'static str {
  if device.is_connected() {
    if let Some( generator ) = device.generator() && generator.is_running() {
      "PLAYING"
    } else {
      "CONNECTED"
    }
  } else {
    "DISCONNECTED"
  }
}