use std::net::SocketAddr;

use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Rect };
use ratatui::style::{ palette::tailwind::SLATE, Style };
use ratatui::text::Line;
use ratatui::widgets::{ Block, Paragraph, Row, StatefulWidget, Table, TableState, Widget };

use crate::device::Device;
use super::{ IsScene, Context, SceneEvent };

const CONNECTED: &str = "Connected";
const DISCONNECTED: &str = "Disconnected";
//const PLAYING: &str = "Playing";

pub struct ListScene {
  device_map_version: usize,
  sorted_device_keys: Vec<SocketAddr>, // Scene cache of sorted device keys
  state: TableState
}

impl ListScene {
  pub fn new() -> Self {
    Self{
      device_map_version: 0,
      sorted_device_keys: Vec::new(),
      state: TableState::new().with_selected( Some( 0 ) )
    }
  }
}

impl IsScene for ListScene {
  fn on_key_press( &'_ mut self, ctx: &mut Context, key: KeyCode ) -> SceneEvent<'_> {
    match key {
      KeyCode::Down => {
        let i = self.state.selected().unwrap_or( 0 ).saturating_add( 1 ) % ctx.device_map().len();
        self.state.select( Some( i ) );
        return SceneEvent::Handled;
      }
      KeyCode::Up => {
        let i = self.state.selected().unwrap_or( 0 ).saturating_sub( 1 ) % ctx.device_map().len();
        self.state.select( Some( i ) );
        return SceneEvent::Handled;
      }
      KeyCode::Enter => {
        if let Some( addr ) = self.state.selected().and_then( |i|{ self.sorted_device_keys.get( i ) } ) {
          return SceneEvent::Select( addr );
        }
      }
      _ => {}
    }

    SceneEvent::NotHandled
  }

  fn render( &mut self, ctx: &Context, area: Rect, buf: &mut Buffer ) {
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
    let constraints = [ Constraint::Length( 25 ), Constraint::Length( 25 ), Constraint::Fill( 1 ) ];

    let rows: Vec<Row> = self.sorted_device_keys
      .iter()
      .filter_map(| addr |{
        if let Some( device ) = ctx.device_map().get( addr ) {
          Some( Row::new([
            addr.to_string(),
            device.info().mac_address().to_string(),
            device_status( device ).to_string()
          ]) )
        } else {
          None
        }
      })
      .collect();

    let table = Table::new( rows, constraints )
      .block( block )
      .header( Row::new(vec![ "Address", "MAC", "Status" ]).style( Style::new().bold() ) )
      .highlight_spacing( ratatui::widgets::HighlightSpacing::Always )
      .highlight_symbol( "> " )
      .row_highlight_style( Style::new().bg( SLATE.c800 ) );

    StatefulWidget::render( table, area, buf, &mut self.state );
  }
}

fn device_status( device: &Device ) -> &str {
  if let Some( _generator ) = device.generator() {
    CONNECTED
  } else {
    DISCONNECTED
  }
}