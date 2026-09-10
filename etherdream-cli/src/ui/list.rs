use std::net::SocketAddr;

use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Rect };
use ratatui::style::{ Color, palette::tailwind::SLATE, Style };
use ratatui::text::Line;
use ratatui::widgets::{ Block, Cell, Paragraph, Row, StatefulWidget, Table, TableState, Widget };

use crate::device;
use crate::scene;
use crate::ui::{ Action, State };

const CONNECTED: &str = " Connected ";
const DISCONNECTED: &str = " Disconnected ";
const PLAYING: &str = " Playing ";

const HIGHLIGHT_STYLE: Style = Style::new().bg( SLATE.c800 );

pub struct ListScene {
  device_addrs: Vec<SocketAddr>,
  table: TableState,
  version: usize
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

    // Update the table state if the discovered device registry has changed.
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
    let body_area = super::common::layout( area, buf, "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." );
    let block = Block::bordered().title( Line::raw( " Etherdream Devices " ).centered() );

    // If there are no devices, render a message saying as such
    if self.table.selected().is_none() {
      Paragraph::new( "(no devices)" ).centered().block( block ).render( area, buf );
      return;
    }

    let rows: Vec<Row> = {
      let state = ctx.state();

      self.device_addrs
        .iter()
        .enumerate()
        .filter_map(|( i, addr )|{
          if let Some( device ) = state.get_devices().get( addr ) {
            let selected = self.table.selected().filter(| si |{ *si == i }).is_some();
            let theme = if selected { HIGHLIGHT_STYLE } else { Style::new() };

            Some( Row::new([
              Cell::new( device.info().ip().to_string() ),
              Cell::new( "-" ),
              Cell::new( device.info().mac_address().to_string() ),
              render_device_status_cell( &device, selected )
            ]).style( theme ) )
          } else {
            None
          }
        })
        .collect()
    };

    let constraints = [
      Constraint::Min( 20 ),
      Constraint::Percentage( 25 ),
      Constraint::Percentage( 25 ),
      Constraint::Fill( 1 ) ];

    let table = Table::new( rows, constraints )
      .block( block )
      .header( Row::new( vec![ "Ip", "Port", "MAC", "Status" ] ).style( Style::new().bold() ) )
      .highlight_spacing( ratatui::widgets::HighlightSpacing::Always )
      .highlight_symbol( "> " );

    StatefulWidget::render( table, body_area, buf, &mut self.table );
  }
}

fn render_device_status_cell<'a>(device: &device::Device, selected: bool ) -> Cell<'a> {
  if device.is_connected() {
    if let Some( generator ) = device.generator() && generator.is_running() {
      Cell::new( PLAYING ).style( Style::new().bg( Color::Green ) )
    } else {
      Cell::new( CONNECTED ).style( Style::new().bg( Color::Yellow ) )
    }
  } else {
    let cell = Cell::new( DISCONNECTED );
    if selected { cell.style( HIGHLIGHT_STYLE ) } else { cell }
  }
}