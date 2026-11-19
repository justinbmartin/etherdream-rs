use std::fmt::Write;

use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::text::Line;
use ratatui::widgets::{ Block, Cell, Padding, Row, StatefulWidget, Table, TableState, Widget };

use crate::app::{ Action, State };
use crate::scene;
use crate::sequencer::Track;
use crate::tracks;
use super::common;

const FOOTER_CONNECTED_MSG: &str = "Use '<Enter>' to play a generator, 'd' to disconnect, 'Esc' to go back.";
const FOOTER_DISCONNECTED_MSG: &str = "Use 'c' to connect, 'Esc' to go back.";

#[derive( Default )]
pub struct DevicesViewScene {
  table: TableState,
  tracks: Vec<tracks::Track>,
  tracks_loaded: bool,

  // `DeviceInfo` string caches
  di_hardware_version: String,
  di_ip: String,
  di_mac_address: String,
  di_max_points_per_second: String,
  di_point_buffer_capacity: String,
  di_software_version: String,

  // Sequencer string caches
  seq_points_buffered: String,
  seq_points_lifetime: String,
  seq_points_per_second: String,

  // Etherdream state cache
  state: etherdream::State
}

impl scene::Scene<State> for DevicesViewScene {
  fn on_enter( &mut self, ctx: &scene::Context<State> ) {
    self.table.select( Some( 0 ) );

    // Clear all cached strings
    self.di_hardware_version.clear();
    self.di_ip.clear();
    self.di_mac_address.clear();
    self.di_max_points_per_second.clear();
    self.di_point_buffer_capacity.clear();
    self.di_software_version.clear();
    self.seq_points_buffered.clear();
    self.seq_points_lifetime.clear();
    self.seq_points_per_second.clear();

    //
    self.state = etherdream::State::default();

    // Since `DeviceInfo` never changes, we load that into our string caches on
    // scene enter.
    if let Some( device_info ) = ctx.read_state().get_current_device() {
      let _ = write!( self.di_hardware_version, "{}", device_info.version().hardware );
      let _ = write!( self.di_ip, "{}", device_info.ip() );
      let _ = write!( self.di_mac_address, "{}", device_info.mac_address() );
      let _ = write!( self.di_max_points_per_second, "{}", device_info.max_points_per_second() );
      let _ = write!( self.di_point_buffer_capacity, "{}", device_info.buffer_capacity() );
      let _ = write!( self.di_software_version, "{}", device_info.version().software );
    }
  }

  fn on_exit( &mut self, _ctx: &scene::Context<State> ) {
    self.tracks_loaded = false
  }

  fn on_key_down( &mut self, key: KeyEvent, ctx: &mut scene::Context<State> ) -> bool {
    match key.code {
      KeyCode::Char( 'c' ) => {
        ctx.invoke( Action::Connect );
        true
      }
      KeyCode::Char( 'd' ) => {
        ctx.invoke( Action::Disconnect );
        true
      }
      KeyCode::Char( 'q' ) => {
        ctx.invoke( Action::Back );
        true
      }
      KeyCode::Char( 's' ) => {
        ctx.invoke( Action::Stop );
        true
      }
      KeyCode::Enter => {
        let track_id =
          if let Some( idx ) = self.table.selected()
            && let Some( sequencer ) = ctx.read_state().get_current_sequencer()
            && let Some( track ) = sequencer.tracks().get( idx )
          {
            Some( *track.id() )
          } else {
            None
          };

        if let Some( track_id ) = track_id {
          ctx.invoke( Action::Play( track_id ) );
        }

        true
      }
      key @ ( KeyCode::Down | KeyCode::Up ) => {
        common::on_key_down_for_table_state( &key, &mut self.table, self.tracks.len() );
        true
      }
      _ => {
        false
      }
    }
  }

  fn on_update( &mut self, ctx: &mut scene::Context<State> ) {
    if let Some( sequencer ) = ctx.read_state().get_current_sequencer() {
      sequencer.clone_state_into( &mut self.state );

      if ! self.tracks_loaded {
        self.tracks.clone_from( sequencer.tracks() );
        self.tracks_loaded = true;
      }
    }

    self.seq_points_buffered.clear();
    let _ = write!( &mut self.seq_points_buffered, "{}", self.state.points_buffered() );

    self.seq_points_lifetime.clear();
    let _ = write!( &mut self.seq_points_lifetime, "{}", self.state.points_lifetime() );

    self.seq_points_per_second.clear();
    let _ = write!( &mut self.seq_points_per_second, "{}", self.state.points_per_second() );
  }

  fn on_draw( &mut self, area: Rect, buf: &mut Buffer, ctx: &scene::Context<State> ) {
    let state = ctx.read_state();
    let sequencer = state.get_current_sequencer();

    let footer_msg = if sequencer.is_some() { FOOTER_CONNECTED_MSG } else { FOOTER_DISCONNECTED_MSG };
    let body_area = common::layout( area, buf, footer_msg );

    let layout = Layout::horizontal([ Constraint::Length( 54 ), Constraint::Fill( 1 ) ]);
    let [ left_area, right_area ] = layout.areas( body_area );

    // Render left pane
    self.draw_device_panel( sequencer.is_some(), sequencer.is_some() && self.state.is_playing(), left_area, buf );

    // Render right pane
    if sequencer.is_some() {
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
  fn draw_device_panel( &self, is_connected: bool, is_playing: bool, area: Rect, buf: &mut Buffer ) {
    let block = Block::bordered()
      .border_style( common::HIGHLIGHT_BORDER_STYLE )
      .padding( Padding::new( 1, 1, 0, 0 ) )
      .title(
        Line::raw( format!( " Device({}) ", self.di_ip ) )
          .centered()
          .style( common::HIGHLIGHT_TEXT_STYLE )
      );

    let [ intrinsics_area, state_area ] = block.inner( area ).layout( &Layout::vertical([
      Constraint::Length( 8 ), Constraint::Fill( 1 ),
    ]) );

    // Render intrinsics
    let intrinsics_block = Block::new()
      .style( common::HIGHLIGHT_BORDER_STYLE )
      .title( Line::styled( "INTRINSICS:", common::HIGHLIGHT_TEXT_STYLE ) );

    let intrinsic_rows = [
      Row::new([ "IP address:", &self.di_ip ]),
      Row::new([ "MAC address:", &self.di_mac_address ]),
      Row::new([ "Hardware version:", &self.di_hardware_version ]),
      Row::new([ "Software version:", &self.di_software_version ]),
      Row::new([ "Point buffer capacity:", &self.di_point_buffer_capacity ]),
      Row::new([ "Max points per second:", &self.di_max_points_per_second ])
    ];

    let intrinsics_table = Table::new( intrinsic_rows, [ Constraint::Length( 23 ), Constraint::Fill( 1 ) ])
      .block( intrinsics_block );

    // Render state
    let state_block = Block::new()
      .style( common::HIGHLIGHT_BORDER_STYLE )
      .title( Line::styled( "STATE:", common::HIGHLIGHT_TEXT_STYLE ) );

    let mut rows = Vec::with_capacity( 50 );
    rows.push( Row::new([ "Connected:", if is_connected { "Yes" } else { "No" }]) );

    if is_connected {
      rows.push( Row::new([ "Playing:", if is_playing { "Yes" } else { "No" } ]) );
      rows.push( Row::new([ "Points Buffered:", &self.seq_points_buffered ]) );
      rows.push( Row::new([ "Points Per Second:", &self.seq_points_per_second ]) );
      rows.push( Row::new([ "Points Lifetime:", &self.seq_points_lifetime ]) );
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