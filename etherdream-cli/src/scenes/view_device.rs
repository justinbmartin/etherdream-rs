use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::text::Line;
use ratatui::widgets::{ Block, Cell, Padding, Row, StatefulWidget, Table, TableState, Widget };

use crate::executors;
use crate::scene;
use crate::sequencer::Sequencer;
use crate::ui::{ Action, State };
use super::common;

const FOOTER_CONNECTED_MSG: &str = "Use '<Enter>' to play a generator, 'd' to disconnect, 'Esc' to go back.";
const FOOTER_DISCONNECTED_MSG: &str = "Use 'c' to connect, 'Esc' to go back.";

pub struct ViewDeviceScene {
  tracks: TableState
}

impl ViewDeviceScene {
  pub fn new() -> Self {
    Self{
      tracks: TableState::new().with_selected( Some( 0 ) )
    }
  }
}

impl scene::Scene<State> for ViewDeviceScene {
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
        ctx.invoke( Action::Play( executors::TrackId::Beam ) );
        true
      }
      key @ ( KeyCode::Down | KeyCode::Up ) => {
        if let Some( sequencer ) = ctx.read_state().get_current_sequencer() {
          common::on_key_down_for_table_state( &key, &mut self.tracks, sequencer.tracks().len() );
        }

        true
      }
      _ => {
        false
      }
    }
  }

  fn on_draw( &mut self, area: Rect, buf: &mut Buffer, ctx: &scene::Context<State> ) {
    let state = ctx.read_state();

    if let Some( device_info ) = state.get_current_device() {
      let sequencer = state.get_current_sequencer();

      let footer_msg = if sequencer.is_some() { FOOTER_CONNECTED_MSG } else { FOOTER_DISCONNECTED_MSG };
      let body_area = common::layout( area, buf, footer_msg );

      let layout = Layout::horizontal([ Constraint::Length( 54 ), Constraint::Fill( 1 ) ]);
      let [ left_area, right_area ] = layout.areas( body_area );

      // Render left pane
      draw_device_panel( device_info, state.get_current_sequencer(), left_area, buf );

      // Render right pane
      if let Some( sequencer ) = sequencer {
        let title = format!(" Tracks [{}] ", sequencer.tracks().len() );
        let block = common::make_block( Some( &title ) );
        let inner_area = block.inner( right_area );

        Widget::render( block, right_area, buf );
        draw_generators_table( &mut self.tracks, sequencer, inner_area, buf );
      } else {
        let block = common::make_block( None );
        let inner_area = block.inner( right_area );

        Widget::render( block, right_area, buf );
        common::draw_centered_line( "Press 'c' to connect.", common::HIGHLIGHT_ROW_SELECTED_STYLE, inner_area, buf );
      }
    } else {
      // something went wrong...
    }
  }
}

pub fn draw_device_panel( device_info: &etherdream::DeviceInfo, sequencer: Option<&Sequencer<executors::Track>>, area: Rect, buf: &mut Buffer ) -> bool {
  let block = Block::bordered()
    .border_style( common::HIGHLIGHT_BORDER_STYLE )
    .padding( Padding::new( 1, 1, 0, 0 ) )
    .title(
      Line::raw( format!( " Device({}) ", device_info.ip() ) )
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
    Row::new([ "IP address:".to_owned(), device_info.ip().to_string() ]),
    Row::new([ "MAC address:".to_owned(), device_info.mac_address().to_string() ]),
    Row::new([ "Hardware version:".to_owned(), device_info.version().hardware.to_string() ]),
    Row::new([ "Software version:".to_owned(), device_info.version().software.to_string() ]),
    Row::new([ "Point buffer capacity:".to_owned(), device_info.buffer_capacity().to_string() ]),
    Row::new([ "Max points per second:".to_owned(), device_info.max_points_per_second().to_string() ])
  ];

  let intrinsics_table = Table::new( intrinsic_rows, [ Constraint::Length( 23 ), Constraint::Fill( 1 ) ])
    .block( intrinsics_block );

  // Render state
  let state_block = Block::new()
    .style( common::HIGHLIGHT_BORDER_STYLE )
    .title( Line::styled( "STATE:", common::HIGHLIGHT_TEXT_STYLE ) );

  let mut rows = Vec::with_capacity( 50 );
  rows.push( Row::new([ "Connected:", if sequencer.is_some() { "Yes" } else { "No" } ]) );


  if let Some( sequencer ) = sequencer {
    rows.push( Row::new([ "Playing:", if sequencer.is_playing() { "Yes" } else { "No" } ]) );
    rows.push( Row::new([ "Points Buffered:".to_owned(), sequencer.state().points_buffered().to_string() ]) );
    rows.push( Row::new([ "Points Per Second:".to_owned(), sequencer.state().points_per_second().to_string() ]) );
    rows.push( Row::new([ "Points Lifetime:".to_owned(), sequencer.state().points_lifetime().to_string() ]) );
  }

  let state_table = Table::new( rows, [ Constraint::Length( 23 ), Constraint::Fill( 1 ) ])
    .block( state_block );

  Widget::render( block, area, buf );
  Widget::render( intrinsics_table, intrinsics_area, buf );
  Widget::render( state_table, state_area, buf );

  sequencer.is_some()
}

fn draw_generators_table( state: &mut TableState, sequencer: &Sequencer<executors::Track>, area: Rect, buf: &mut Buffer ) {
  const CONSTRAINTS: [Constraint; 2] = [ Constraint::Min( 30 ), Constraint::Fill( 1 ) ];
  let mut rows = Vec::with_capacity( sequencer.tracks().len() );
  let selected_idx = state.selected().unwrap_or( 0 );

  for ( i, track ) in sequencer.tracks().iter().enumerate() {
    let style = if selected_idx == i { common::HIGHLIGHT_ROW_SELECTED_STYLE } else { common::HIGHLIGHT_TEXT_STYLE };
    rows.push( Row::new([ Cell::new( track.name() ), Cell::new( "" ) ]).style( style ) );
  }

  let table = Table::new( rows, CONSTRAINTS )
    .header( Row::new([ "NAME", "STATUS" ]).style( common::TABLE_HEADER_STYLE ) );

  StatefulWidget::render( table, area, buf, state );
}