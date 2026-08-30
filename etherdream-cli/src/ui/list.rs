use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::style::{ Color, palette::tailwind::SLATE, Style };
use ratatui::text::Line;
use ratatui::widgets::{ Block, Cell, Paragraph, Row, StatefulWidget, Table, TableState, Widget };

use crate::device;
use crate::scene;
use crate::ui::{ Action, State };

const CONNECTED: &str = " Connected ";
const DISCONNECTED: &str = "Disconnected";
const PLAYING: &str = " Playing ";

const HIGHLIGHT_STYLE: Style = Style::new().bg( SLATE.c800 );

pub struct ListScene {
  device_map_version: usize,
  selected: Option<usize>,
  sorted_device_keys: Vec<usize>, // Scene cache of sorted device id's
  table: TableState
}

impl ListScene {
  pub fn new() -> Self {
    Self{
      device_map_version: 0,
      selected: None,
      sorted_device_keys: Vec::new(),
      table: TableState::new().with_selected( Some( 0 ) )
    }
  }
}

impl scene::Scene<State> for ListScene {
  fn on_key_down( &mut self, key: KeyEvent, ctx: &mut scene::Context<State> ) -> bool {
    match key.code {
      dir @ ( KeyCode::Up | KeyCode::Down ) => {
        let devices_count = ctx.state().get_device_map().len();

        if devices_count > 0 {
          let selected = self.selected.unwrap_or( 0 );
          let i = if dir == KeyCode::Up { selected.saturating_sub( 1 ) } else { selected.saturating_add( 1 ) };
          self.table.select( Some( i % devices_count ) );
        }

        return true;
      }
      KeyCode::Enter => {
        if let Some( device_id ) = self.table.selected().and_then(| i |{ self.sorted_device_keys.get( i ) }) {
          ctx.invoke( Action::SelectDevice( *device_id ) );
          self.selected = Some( *device_id );
          return true;
        }
      }
      _ => {}
    }

    false
  }

  fn on_update( &mut self, ctx: &mut scene::Context<State> ) {
    let state = ctx.state();
    let devices = state.get_device_map();

    // Refresh our local sorted device cache if the remote device map has changed
    if devices.version() != self.device_map_version {
      self.sorted_device_keys = devices
        .iter()
        .map( |( &addr, _ )|{ addr } )
        .collect();

      self.sorted_device_keys.sort();
      self.device_map_version = devices.version();
    }
  }

  fn on_draw( &mut self, area: Rect, buf: &mut Buffer, ctx: &scene::Context<State> ) {
    let body_area = super::common::layout( area, buf, "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." );
    let block = Block::bordered().title( Line::raw( " Etherdream Devices " ).centered() );

    // If there are no devices, render a message saying as such
    if self.sorted_device_keys.is_empty() {
      Paragraph::new( "(no devices)" ).centered().block( block ).render( area, buf );
      return;
    }

    // Render our table of discovered devices
    let constraints = [
      Constraint::Length( 20 ),
      Constraint::Length( 10 ),
      Constraint::Length( 30 ),
      Constraint::Fill( 1 ) ];

    let rows: Vec<Row> = {
      let state = ctx.state();

      self.sorted_device_keys
        .iter()
        .enumerate()
        .filter_map(|( i, id )|{
          if let Some( device ) = state.get_device_map().get( *id ) {
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

    let table = Table::new( rows, constraints )
      .block( block )
      .header( Row::new(vec![ "Ip", "Port", "MAC", "Status" ]).style( Style::new().bold() ) )
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