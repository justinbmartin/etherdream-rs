use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::style::{ Color, Style };
use ratatui::text::Line;
use ratatui::widgets::{ Block, Padding, Paragraph, Row, Table, Widget };

use crate::device::Device;
use crate::scene;
use crate::ui::State;

pub const INFO_TEXT_STYLE: Style = Style::new().fg( Color::Rgb( 150, 150, 150 ) );
pub const HIGHLIGHT_COLOR: Color = Color::Rgb( 232, 195, 0 );
pub const HIGHLIGHT_BORDER_STYLE: Style = Style::new().fg( HIGHLIGHT_COLOR );
pub const HIGHLIGHT_ROW_SELECTED_STYLE: Style = Style::new().bg( HIGHLIGHT_COLOR ).bold().fg( Color::Black );
pub const HIGHLIGHT_TEXT_STYLE: Style = Style::new().bold().fg( HIGHLIGHT_COLOR );
pub const TABLE_HEADER_STYLE: Style = Style::new().bold().fg( HIGHLIGHT_COLOR );

pub fn layout( area: Rect, buf: &mut Buffer, footer_msg: &str ) -> Rect {
  let layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
  let [ body, footer ] = area.layout( &layout );

  Paragraph::new( footer_msg ).style( INFO_TEXT_STYLE ).centered().render( footer, buf );
  body
}

pub fn make_block( title: Option<&'_ str> ) -> Block<'_> {
  let block = Block::bordered()
    .border_style( HIGHLIGHT_BORDER_STYLE )
    .padding( Padding::new( 1, 1, 0, 0 ) );

  if let Some( title ) = title {
    block.title( Line::styled( title, HIGHLIGHT_TEXT_STYLE ).centered() )
  } else {
    block
  }
}

pub fn draw_device_panel( device: &Device, area: Rect, buf: &mut Buffer ) -> bool {
  let block = Block::bordered()
    .border_style( HIGHLIGHT_BORDER_STYLE )
    .padding( Padding::new( 1, 1, 0, 0 ) )
    .title(
      Line::raw( format!( " Device({}) ", device.address().ip() ) )
        .centered()
        .style( HIGHLIGHT_TEXT_STYLE )
    );

  let [ intrinsics_area, state_area ] = block.inner( area ).layout( &Layout::vertical([
    Constraint::Length( 8 ), Constraint::Fill( 1 ),
  ]) );

  // Render intrinsics
  let intrinsics_block = Block::new()
    .style( HIGHLIGHT_BORDER_STYLE )
    .title( Line::styled( "INTRINSICS:", HIGHLIGHT_TEXT_STYLE ) );

    let intrinsic_rows = [
      Row::new([ "IP address:".to_owned(), device.info().ip().to_string() ]),
      Row::new([ "MAC address:".to_owned(), device.info().mac_address().to_string() ]),
      Row::new([ "Hardware version:".to_owned(), device.info().version().hardware.to_string() ]),
      Row::new([ "Software version:".to_owned(), device.info().version().software.to_string() ]),
      Row::new([ "Point buffer capacity:".to_owned(), device.info().buffer_capacity().to_string() ]),
      Row::new([ "Max points per second:".to_owned(), device.info().max_points_per_second().to_string() ])
    ];

    let intrinsics_table = Table::new( intrinsic_rows, [ Constraint::Length( 23 ), Constraint::Fill( 1 ) ])
      .block( intrinsics_block );

    // Render state
    let state_block = Block::new()
      .style( HIGHLIGHT_BORDER_STYLE )
      .title( Line::styled( "STATE:", HIGHLIGHT_TEXT_STYLE ) );

    let mut rows = Vec::with_capacity( 50 );
    rows.push( Row::new([ "Connected:", if device.is_connected() { "Yes" } else { "No" } ]) );

    if device.is_connected() {
      let mut state = etherdream::State::default();
      device.state( &mut state );

      rows.push( Row::new([ "Points Buffered:".to_owned(), state.points_buffered().to_string() ]) );
      rows.push( Row::new([ "Points Per Second:".to_owned(), state.points_per_second().to_string() ]) );
      rows.push( Row::new([ "Points Lifetime:".to_owned(), state.points_lifetime().to_string() ]) );
    }

    let state_table = Table::new( rows, [ Constraint::Length( 23 ), Constraint::Fill( 1 ) ])
      .block( state_block );

    Widget::render( block, area, buf );
    Widget::render( intrinsics_table, intrinsics_area, buf );
    Widget::render( state_table, state_area, buf );

    device.is_connected()
}