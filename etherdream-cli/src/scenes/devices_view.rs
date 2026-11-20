use std::fmt::Write;

use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::text::Line;
use ratatui::widgets::{ Block, Cell, Padding, Row, StatefulWidget, Table, TableState, Widget };

use crate::app;
use crate::scene;
use crate::sequencer::Track;
use crate::tracks;
use super::common;

#[derive( Default )]
pub struct DevicesViewScene {
  device: etherdream::State,
  device_title: String,
  is_connected: bool,
  table: TableState,
  tracks: Vec<tracks::Track>,
  tracks_loaded: bool,
  tracks_title: String,

  // String caches for `etherdream::DeviceInfo`
  hardware_version: String,
  ip: String,
  mac_addr: String,
  max_points_per_second: String,
  point_buffer_capacity: String,
  software_version: String,

  // String caches for `etherdream::State`
  points_buffered: String,
  points_lifetime: String,
  points_per_second: String,
}

impl scene::Scene<app::State> for DevicesViewScene {
  fn on_enter( &mut self, ctx: &scene::Context<app::State> ) {
    // Reset all properties
    self.device = etherdream::State::default();
    self.device_title.clear();
    self.hardware_version.clear();
    self.ip.clear();
    self.is_connected = false;
    self.mac_addr.clear();
    self.max_points_per_second.clear();
    self.point_buffer_capacity.clear();
    self.points_buffered.clear();
    self.points_lifetime.clear();
    self.points_per_second.clear();
    self.software_version.clear();
    self.table.select( Some( 0 ) );
    self.tracks.clear();
    self.tracks_loaded = false;

    // Reset the tracks title
    self.tracks_title.clear();
    set_tracks_title( &mut self.tracks_title, &self.tracks );

    // Since `DeviceInfo` never changes, we load that into our string caches on
    // scene enter.
    if let Some( device_info ) = ctx.read_state().get_current_device() {
      let _ = write!( self.device_title, " Device({}) ", device_info.ip() );
      let _ = write!( self.hardware_version, "{}", device_info.version().hardware );
      let _ = write!( self.ip, "{}", device_info.ip() );
      let _ = write!( self.mac_addr, "{}", device_info.mac_address() );
      let _ = write!( self.max_points_per_second, "{}", device_info.max_points_per_second() );
      let _ = write!( self.point_buffer_capacity, "{}", device_info.buffer_capacity() );
      let _ = write!( self.software_version, "{}", device_info.version().software );
    }
  }

  fn on_key_down( &mut self, key: KeyEvent, ctx: &mut scene::Context<app::State> ) -> bool {
    match key.code {
      KeyCode::Char( 'c' ) => {
        ctx.invoke( app::Action::Connect );
        true
      }
      KeyCode::Char( 'd' ) => {
        ctx.invoke( app::Action::Disconnect );
        true
      }
      KeyCode::Char( 'q' ) => {
        ctx.invoke( app::Action::Back );
        true
      }
      KeyCode::Char( 'p' ) => {
        if let Some( track ) = self.table.selected().and_then(| idx | self.tracks.get( idx ) ) {
          ctx.invoke( app::Action::Play( *track.id() ) );
        }

        true
      }
      KeyCode::Char( 's' ) => {
        ctx.invoke( app::Action::Stop );
        true
      }
      key => {
        common::on_key_down_for_table_state( &key, &mut self.table, self.tracks.len() )
      }
    }
  }

  fn on_update( &mut self, ctx: &mut scene::Context<app::State> ) {
    if let Some( sequencer ) = ctx.read_state().get_current_sequencer() {
      sequencer.clone_state_into( &mut self.device );
      self.is_connected = true;

      if ! self.tracks_loaded {
        self.tracks.clone_from( sequencer.tracks() );
        self.tracks_loaded = true;

        set_tracks_title( &mut self.tracks_title, &self.tracks );
      }
    } else {
      self.device = etherdream::State::default();
      self.is_connected = false;
      self.tracks_loaded = false;
    }

    //
    self.points_buffered.clear();
    let _ = write!( &mut self.points_buffered, "{}", self.device.points_buffered() );

    self.points_lifetime.clear();
    let _ = write!( &mut self.points_lifetime, "{}", self.device.points_lifetime() );

    self.points_per_second.clear();
    let _ = write!( &mut self.points_per_second, "{}", self.device.points_per_second() );
  }

  fn on_draw( &mut self, area: Rect, buf: &mut Buffer, _ctx: &scene::Context<app::State> ) {
    let footer_msg = if self.is_connected {
        "Use '<Enter>' to play a generator, 'd' to disconnect, 'Esc' to go back."
      } else {
        "Use 'c' to connect, 'Esc' to go back."
      };

    let body_area = common::layout( area, buf, footer_msg );
    let layout = Layout::horizontal([ Constraint::Length( 54 ), Constraint::Fill( 1 ) ]);
    let [ left_area, right_area ] = layout.areas( body_area );

    // Render left pane
    self.draw_device_panel( self.device.is_playing(), left_area, buf );

    // Render right pane
    if self.is_connected {
      let title = format!(" Tracks [{}] ", self.tracks.len() );
      let block = common::make_block( Some( &title ) );
      let inner_area = block.inner( right_area );

      Widget::render( block, right_area, buf );
      draw_generators_table( &mut self.table, &self.tracks, inner_area, buf );
    } else {
      let block = common::make_block( None );
      let inner_area = block.inner( right_area );

      Widget::render( block, right_area, buf );
      common::draw_centered_line( "Press 'c' to connect.", common::HIGHLIGHT_ROW_SELECTED_STYLE, inner_area, buf );
    }
  }
}

impl DevicesViewScene {
  fn draw_device_panel( &self, is_playing: bool, area: Rect, buf: &mut Buffer ) {
    let block = Block::bordered()
      .border_style( common::HIGHLIGHT_BORDER_STYLE )
      .padding( Padding::new( 1, 1, 0, 0 ) )
      .title( Line::raw( &self.device_title ).centered().style( common::HIGHLIGHT_TEXT_STYLE ) );

    let [ intrinsics_area, state_area ] = block.inner( area )
      .layout( &Layout::vertical([ Constraint::Length( 8 ), Constraint::Fill( 1 ) ]) );

    // Render intrinsics
    let intrinsics_block = Block::new()
      .style( common::HIGHLIGHT_BORDER_STYLE )
      .title( Line::styled( "INTRINSICS:", common::HIGHLIGHT_TEXT_STYLE ) );

    let intrinsic_rows = [
      Row::new([ "IP address:", &self.ip ]),
      Row::new([ "MAC address:", &self.mac_addr ]),
      Row::new([ "Hardware version:", &self.hardware_version ]),
      Row::new([ "Software version:", &self.software_version ]),
      Row::new([ "Point buffer capacity:", &self.point_buffer_capacity ]),
      Row::new([ "Max points per second:", &self.max_points_per_second ])
    ];

    let intrinsics_table = Table::new( intrinsic_rows, [ Constraint::Length( 23 ), Constraint::Fill( 1 ) ])
      .block( intrinsics_block );

    // Render state
    let state_block = Block::new()
      .style( common::HIGHLIGHT_BORDER_STYLE )
      .title( Line::styled( "STATE:", common::HIGHLIGHT_TEXT_STYLE ) );

    let mut rows = Vec::with_capacity( 50 );
    rows.push( Row::new([ "Connected:", if self.is_connected { "Yes" } else { "No" }]) );

    if self.is_connected {
      rows.push( Row::new([ "Playing:", if is_playing { "Yes" } else { "No" } ]) );
      rows.push( Row::new([ "Points Buffered:", &self.points_buffered ]) );
      rows.push( Row::new([ "Points Per Second:", &self.points_per_second ]) );
      rows.push( Row::new([ "Points Lifetime:", &self.points_lifetime ]) );
    }

    let state_table = Table::new( rows, [ Constraint::Length( 23 ), Constraint::Fill( 1 ) ])
      .block( state_block );

    Widget::render( block, area, buf );
    Widget::render( intrinsics_table, intrinsics_area, buf );
    Widget::render( state_table, state_area, buf );
  }
}

fn draw_generators_table( state: &mut TableState, tracks: &Vec<tracks::Track>, area: Rect, buf: &mut Buffer ) {
  const CONSTRAINTS: [Constraint; 2] = [ Constraint::Min( 30 ), Constraint::Fill( 1 ) ];
  let mut rows = Vec::with_capacity( tracks.len() );
  let selected_idx = state.selected().unwrap_or( 0 );

  for ( i, track ) in tracks.iter().enumerate() {
    let style = if selected_idx == i { common::HIGHLIGHT_ROW_SELECTED_STYLE } else { common::HIGHLIGHT_TEXT_STYLE };
    rows.push( Row::new([ Cell::new( track.name() ), Cell::new( "" ) ]).style( style ) );
  }

  let table = Table::new( rows, CONSTRAINTS )
    .header( Row::new([ "NAME", "STATUS" ]).style( common::TABLE_HEADER_STYLE ) );

  StatefulWidget::render( table, area, buf, state );
}

fn set_tracks_title( tracks_title: &mut String, tracks: &Vec<tracks::Track> ) {
  let _ = write!( tracks_title, " Tracks [{}] ", tracks.len() );
}