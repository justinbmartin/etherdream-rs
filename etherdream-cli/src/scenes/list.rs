use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Rect };
use ratatui::style::{ Color, palette::tailwind::SLATE, Style };
use ratatui::text::Line;
use ratatui::widgets::{ Block, Cell, Paragraph, Row, StatefulWidget, Table, TableState, Widget };

use crate::app;
use crate::device;
use crate::scene;

const CONNECTED: &str = " Connected ";
const DISCONNECTED: &str = "Disconnected";
const PLAYING: &str = " Playing ";

const HIGHLIGHT_STYLE: Style = Style::new().bg( SLATE.c800 );

pub struct ListScene {
  devices: device::ReadOnlyDeviceMap,
  device_map_version: usize,
  selected: Option<usize>,
  sorted_device_keys: Vec<usize>, // Scene cache of sorted device id's
  state: TableState
}

impl ListScene {
  pub fn new( devices: device::ReadOnlyDeviceMap ) -> Self {
    Self{
      device_map_version: 0,
      devices,
      selected: None,
      sorted_device_keys: Vec::new(),
      state: TableState::new().with_selected( Some( 0 ) )
    }
  }
}

impl scene::Scene<app::Action> for ListScene {
  fn on_key_down( &mut self, key: KeyEvent ) -> bool {
    match key.code {
      dir @ ( KeyCode::Up | KeyCode::Down ) => {
        let devices_count = self.devices.read().len();

        if devices_count > 0 {
          let selected = self.selected.unwrap_or( 0 );
          let i = if dir == KeyCode::Up { selected.saturating_sub( 1 ) } else { selected.saturating_add( 1 ) };
          self.state.select( Some( i % devices_count ) );
        }

        return true;
      }
      KeyCode::Enter => {
        if let Some( id ) = self.selected.and_then(| i |{ self.sorted_device_keys.get( i ) }) {
          self.selected = Some( *id );
          return true;
        }
      }
      _ => {}
    }

    false
  }

  fn on_update( &mut self ) -> scene::Event<app::Action> {
    let devices = self.devices.read();

    // Refresh our local sorted device cache if the remote device map has changed
    if devices.version() != self.device_map_version {
      self.sorted_device_keys = devices
        .iter()
        .map( |( &addr, _ )|{ addr } )
        .collect();

      self.sorted_device_keys.sort();
      self.device_map_version = devices.version();
    }

    if let Some( id ) = self.selected.take() {
      scene::Event::Change( app::Action::Device( id ) )
    } else {
      scene::Event::Noop
    }
  }

  fn on_draw( &mut self, area: Rect, buf: &mut Buffer ) {
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

    let rows: Vec<Row> = self.sorted_device_keys
      .iter()
      .enumerate()
      .filter_map(|( i, id )|{
        if let Some( device ) = self.devices.read().get( *id ) {
          let selected = self.state.selected().filter(| si |{ *si == i }).is_some();
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
      .collect();

    let table = Table::new( rows, constraints )
      .block( block )
      .header( Row::new(vec![ "Ip", "Port", "MAC", "Status" ]).style( Style::new().bold() ) )
      .highlight_spacing( ratatui::widgets::HighlightSpacing::Always )
      .highlight_symbol( "> " );

    StatefulWidget::render( table, area, buf, &mut self.state );
  }
}

fn render_device_status_cell<'a>( device: &device::Device, selected: bool ) -> Cell<'a> {
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