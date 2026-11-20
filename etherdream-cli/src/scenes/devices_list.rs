use std::collections::BTreeMap;
use std::net::IpAddr;

use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::widgets::{ Cell, Row, StatefulWidget, Table, TableState, Widget };

use crate::app;
use crate::scene;
use super::common;

pub struct DevicesListScene {
  device_cache: Vec<Device>,
  table: TableState,
  version: usize,
}

impl DevicesListScene {
  pub fn new() -> Self {
    Self{
      device_cache: Vec::new(),
      table: TableState::new(),
      version: 0
    }
  }
}

impl scene::Scene<app::State> for DevicesListScene {
  fn on_key_down( &mut self, key: KeyEvent, ctx: &mut scene::Context<app::State> ) -> bool {
    match key.code {
      KeyCode::Enter => {
        if let Some( ip ) = self.table.selected().and_then(| i |{ self.device_cache.get( i ).map(| d | &d.ip ) }) {
          ctx.invoke( app::Action::SelectDevice( *ip ) );
        }

        true
      }
      key => {
        common::on_key_down_for_table_state( &key, &mut self.table, self.device_cache.len() )
      }
    }
  }

  fn on_update( &mut self, ctx: &mut scene::Context<app::State> ) {
    let state = ctx.read_state();

    if state.devices_version() != self.version {
      let mut updated_idx = None::<usize>;

      let mut addrs: BTreeMap<IpAddr,Device> = state.devices().iter().map(|( &ip, &d )| Device{
        ip,
        ip_str: ip.to_string(),
        mac_str: d.mac_address().to_string()
      }).collect();

      if ! addrs.is_empty() {
        addrs.sort_by_key(| d | d.ip );

        if let Some( idx ) = self.table.selected() {
          if let Some( device_cache ) = self.device_cache.get( idx ) && let Ok( idx ) = addrs.binary_search_by_key( &device_cache.ip, | d | d.ip ) {
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

      self.device_cache = addrs;
      self.table.select( updated_idx );
      self.version = state.devices_version();
    }
  }

  fn on_draw( &mut self, area: Rect, buf: &mut Buffer, ctx: &scene::Context<app::State> ) {
    let layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
    let [ body_area, footer_area ] = area.layout( &layout );

    let title = format!( " Etherdream Devices [{}] ", self.device_cache.len() );
    let block = common::make_block( Some( &title ) );
    let inner_area = block.inner( body_area );

    if self.device_cache.is_empty() {
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
    let mut rows = Vec::with_capacity( self.device_cache.len() );

    if let Some( selected_idx ) = self.table.selected() {
      let state = ctx.read_state();

      for ( i, device_cache ) in self.device_cache.iter().enumerate() {
        let style = if selected_idx == i { common::HIGHLIGHT_ROW_SELECTED_STYLE } else { common::HIGHLIGHT_TEXT_STYLE };

        let sequencer_status = match state.get_current_sequencer() {
          Some( sequencer ) => if sequencer.is_playing() { "PLAYING" } else { "CONNECTED" },
          None => "DISCONNECTED"
        };

        rows.push(
          Row::new([
            Cell::new( device_cache.ip_str.as_str() ),
            Cell::new( device_cache.mac_str.as_str() ),
            Cell::new( sequencer_status )
          ]).style( style )
        );
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
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device

struct Device {
  ip: IpAddr,
  ip_str: String,
  mac_str: String
}