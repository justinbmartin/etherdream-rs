use std::net::SocketAddr;

use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Rect };
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{ Block, Cell, Padding, Row, StatefulWidget, Table, TableState };

use crate::device::Device;
use crate::scene;
use crate::ui::{ Action, State };
use super::common;

const CONNECTED: &str = "Connected";
const DISCONNECTED: &str = "Disconnected";
const PLAYING: &str = "Playing";
const TABLE_HEADERS: &[&str] = &[ "IP", "PORT", "MAC", "STATUS" ];

pub struct ListScene {
  device_addrs: Vec<SocketAddr>,
  table: TableState,
  table_rows: Vec<Row<'static>>,
  table_row_style: Style,
  version: usize,
}

impl ListScene {
  pub fn new() -> Self {
    Self{
      device_addrs: Vec::new(),
      table: TableState::new(),
      table_rows: Vec::with_capacity( 10 ),
      table_row_style: Style::new(),
      version: 0
    }
  }
}

impl scene::Scene<State> for ListScene {
  fn on_key_down( &mut self, key: KeyEvent, ctx: &mut scene::Context<State> ) -> bool {
    match key.code {
      dir @ ( KeyCode::Up | KeyCode::Down ) => {
        let devices_count = ctx.state().get_devices().len();

        if devices_count > 0 && let Some( selected_idx ) = self.table.selected() {
          let i = if dir == KeyCode::Up { selected_idx.saturating_sub( 1 ) } else { selected_idx.saturating_add( 1 ) };
          self.table.select( Some( i % devices_count ) );
        }

        return true;
      }
      KeyCode::Enter => {
        if let Some( device_id ) = self.table.selected().and_then(| i |{ self.device_addrs.get( i ) }) {
          ctx.invoke( Action::SelectDevice( *device_id ) );
          return true;
        }
      }
      _ => {}
    }

    false
  }

  fn on_update( &mut self, ctx: &mut scene::Context<State> ) {
    let state = ctx.state();
    let devices = state.get_devices();

    if devices.version() != self.version {
      let mut addrs: Vec<SocketAddr> = devices.iter().map(|( &d, _ )| d ).collect();
      let mut updated_idx = None::<usize>;

      if ! addrs.is_empty() {
        addrs.sort();

        if let Some( previous_idx ) = self.table.selected() {
          if let Some( addr ) = self.device_addrs.get( previous_idx ) && let Ok( idx ) = addrs.binary_search( addr ) {
            // The device index may have changed. Update the device index to reflect any new position.
            updated_idx = Some( idx );
          } else {
            // The previously selected device no longer exists.
            updated_idx = Some( previous_idx.min( addrs.len() ) );
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
    let body_area = common::layout( area, buf, "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." );

    let block = Block::bordered()
      .border_style( common::HIGHLIGHT_BORDER_STYLE )
      .padding( Padding::new( 1, 1, 0, 0 ) )
      .title(
        Line::raw( format!( " Etherdream Devices [{}] ", self.device_addrs.len() ) )
          .centered()
          .style( common::HIGHLIGHT_TEXT_STYLE )
      );

    self.table_rows.clear();

    if let Some( selected_idx ) = self.table.selected() {
      let state = ctx.state();

      self.table_rows = self.device_addrs
        .iter()
        .enumerate()
        .filter_map(|( i, addr )|{
          if let Some( device ) = state.get_devices().get( addr ) {
            self.table_row_style = if selected_idx == i { common::HIGHLIGHT_ROW_SELECTED_STYLE } else { common::HIGHLIGHT_TEXT_STYLE };

            Some(
              Row::new([
                Cell::new( device.info().ip().to_string() ),
                Cell::new( "-" ),
                Cell::new( device.info().mac_address().to_string() ),
                Cell::new( device_status( &device ) )
              ]).style( self.table_row_style )
            )
          } else {
            None
          }
        })
        .collect()
    }

    const CONSTRAINTS: [Constraint; 4] = [
      Constraint::Min( 20 ),
      Constraint::Percentage( 25 ),
      Constraint::Percentage( 25 ),
      Constraint::Fill( 1 ) ];

    Table::new( self.table_rows.iter().cloned(), CONSTRAINTS )
      .block( block )
      .header( Row::new( TABLE_HEADERS.iter().cloned() ).style( common::TABLE_HEADER_STYLE ) )
      .render( body_area, buf, &mut self.table );
  }
}

fn device_status( device: &Device ) -> &'static str {
  if device.is_connected() {
    if let Some( generator ) = device.generator() && generator.is_running() {
      PLAYING
    } else {
      CONNECTED
    }
  } else {
    DISCONNECTED
  }
}