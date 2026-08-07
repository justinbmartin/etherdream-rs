use std::sync::{ Arc, Mutex };

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

struct ListState {
  pub device_map_version: usize,
  pub selected: Option<usize>,
  pub sorted_device_keys: Vec<usize>, // Scene cache of sorted device id's
  pub state: TableState
}

impl Default for ListState {
  fn default() -> Self {
    Self{
      device_map_version: 0,
      selected: None,
      sorted_device_keys: Vec::new(),
      state: TableState::new().with_selected( Some( 0 ) )
    }
  }
}

pub fn make_list_scene_def( devices: Arc<Mutex<app::Devices>> ) -> scene::SceneDefinition<app::Event> {
  let state = Arc::new( Mutex::new( ListState::default() ) );

  scene::SceneDefinition{
    on_key_down: {
      let devices = devices.clone();
      let state = state.clone();

      Some( Box::new( move | e: KeyEvent |{
        let d = &devices.lock().unwrap().devices;
        let s = &mut *state.lock().unwrap();

        Box::pin( on_key_down( s, d, e ) )
      }) )
    },
    on_update: {
      let devices = devices.clone();
      let state = state.clone();

      let d = &devices.lock().unwrap().devices;
      let s = &mut *state.lock().unwrap();

      Some( Box::new( move ||{ Box::pin( on_update( s, d ) ) }) )
    },
    on_render: {
      let devices = devices.clone();
      let state = state.clone();

      let d = &devices.lock().unwrap().devices;
      let s = &mut *state.lock().unwrap();

      Box::new( move | area, buf |{ on_render( s, d, area, buf ) })
    }
  }
}

async fn on_key_down( state: &mut ListState, devices: &device::DeviceMap, key: KeyEvent ) -> bool {
  match key.code {
    KeyCode::Down => {
      let i = state.selected.unwrap_or( 0 ).saturating_add( 1 ) % devices.len();
      state.state.select( Some( i ) );
      return true;
    }
    KeyCode::Up => {
      let i = state.selected.unwrap_or( 0 ).saturating_sub( 1 ) % devices.len();
      state.state.select( Some( i ) );
      return true;
    }
    KeyCode::Enter => {
      if let Some( id ) = state.selected.and_then(| i |{ state.sorted_device_keys.get( i ) }) {
        state.selected = Some( *id );
        return true;
      }
    }
    _ => {}
  }

  false
}

async fn on_update( state: &mut ListState, devices: &device::DeviceMap ) -> scene::Event<app::Event> {
  // Refresh our local sorted device cache if the remote device map has changed
  if devices.version() != state.device_map_version {
    state.sorted_device_keys = devices
      .iter()
      .map( |( &addr, _ )|{ addr } )
      .collect();

    state.sorted_device_keys.sort();
    state.device_map_version = devices.version();
  }

  if let Some( id ) = state.selected.take() {
    scene::Event::Change( app::Event::Connect( id ) )
  } else {
    scene::Event::NoChange
  }
}

// TODO: make ListState immutable (move to update)
fn on_render( state: &mut ListState, devices: &device::DeviceMap, area: Rect, buf: &mut Buffer ) {
  let block = Block::bordered().title( Line::raw( " Etherdream Devices " ).centered() );

  // If there are no devices, render a message saying as such
  if state.sorted_device_keys.is_empty() {
    Paragraph::new( "(no devices)" ).centered().block( block ).render( area, buf );
    return;
  }

  // Render our table of discovered devices
  let constraints = [
    Constraint::Length( 20 ),
    Constraint::Length( 10 ),
    Constraint::Length( 30 ),
    Constraint::Fill( 1 ) ];

  let rows: Vec<Row> = state.sorted_device_keys
    .iter()
    .enumerate()
    .filter_map(|( i, id )|{
      if let Some( device ) = devices.get( *id ) {
        let selected = state.state.selected().filter(| si |{ *si == i }).is_some();
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

  StatefulWidget::render( table, area, buf, &mut state.state );
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