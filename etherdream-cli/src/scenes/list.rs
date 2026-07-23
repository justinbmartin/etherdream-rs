use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Rect };
use ratatui::style::{ Color, palette::tailwind::SLATE, Style };
use ratatui::text::Line;
use ratatui::widgets::{ Block, Cell, Paragraph, Row, StatefulWidget, Table, TableState, Widget };

use crate::actions::AssignCurrentDevice;
use crate::app;
use crate::device::Device;
use crate::scene::{ self, Scene };
use super::SceneContext;

const CONNECTED: &str = " Connected ";
const DISCONNECTED: &str = "Disconnected";
const PLAYING: &str = " Playing ";

const HIGHLIGHT_STYLE: Style = Style::new().bg( SLATE.c800 );

const LIST_SCENE_ID: &str = "list";

pub struct ListScene {
  assign_current_device: AssignCurrentDevice,
  device_map_version: usize,
  sorted_device_keys: Vec<usize>, // Scene cache of sorted device id's
  state: TableState
}

impl ListScene {
  fn get_selected_device_id( &self ) -> Option<usize> {
    self.state.selected().and_then(| i |{ self.sorted_device_keys.get( i ) }).copied()
  }
}

impl ListScene {
  pub fn new( assign_current_device: AssignCurrentDevice ) -> Self {
    Self{
      assign_current_device,
      device_map_version: 0,
      sorted_device_keys: Vec::new(),
      state: TableState::new().with_selected( Some( 0 ) )
    }
  }
}

impl Scene<SceneContext,app::Event> for ListScene {
  fn on_key_down( &mut self, ctx: &mut SceneContext, key: KeyEvent ) -> scene::Event<app::Event> {
    match key.code {
      KeyCode::Down => {
        let i = self.state.selected().unwrap_or( 0 ).saturating_add( 1 ) % ctx.device_map().len();
        self.state.select( Some( i ) );
        return scene::Event::Handled;
      }
      KeyCode::Up => {
        let i = self.state.selected().unwrap_or( 0 ).saturating_sub( 1 ) % ctx.device_map().len();
        self.state.select( Some( i ) );
        return scene::Event::Handled;
      }
      KeyCode::Enter => {
        if let Some( id ) = self.get_selected_device_id() {
          return scene::Event::Custom( app::Event::Connect( id ) );
        }
      }
      _ => {}
    }

    scene::Event::NotHandled
  }

  fn on_render( &mut self, ctx: &SceneContext, area: Rect, buf: &mut Buffer ) {
    let block = Block::bordered().title( Line::raw( " Etherdream Devices " ).centered() );

    // Refresh our local sorted device cache if the remote device map has changed
    if ctx.device_map().version() != self.device_map_version {
      self.sorted_device_keys = ctx.device_map()
        .iter()
        .map( |( &addr, _ )|{ addr } )
        .collect();

      self.sorted_device_keys.sort();
      self.device_map_version = ctx.device_map().version();
    }

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
        if let Some( device ) = ctx.device_map().get( *id ) {
          let selected = self.state.selected().filter(| si |{ *si == i }).is_some();
          let theme = if selected { HIGHLIGHT_STYLE } else { Style::new() };

          Some( Row::new([
            Cell::new( device.info().ip().to_string() ),
            Cell::new( "-" ),
            Cell::new( device.info().mac_address().to_string() ),
            render_device_status_cell( device, selected )
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

fn render_device_status_cell<'a>( device: &Device, selected: bool ) -> Cell<'a> {
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