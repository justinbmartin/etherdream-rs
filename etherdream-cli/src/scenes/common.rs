use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::{ Alignment, Constraint, Layout, Rect };
use ratatui::style::{ Color, Style };
use ratatui::text::Line;
use ratatui::widgets::{ Block, Padding, Paragraph, TableState, Widget };

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

pub fn draw_footer( msg: &str, area: Rect, buf: &mut Buffer ) {
  Paragraph::new( msg ).style( INFO_TEXT_STYLE ).centered().render( area, buf );
}

pub fn draw_centered_line( msg: &str, style: Style, area: Rect, buf: &mut Buffer ) {
  let line = Line::styled( msg, style ).alignment( Alignment::Center );

  let center_y = area.y + ( area.height / 2 );
  let centered_area = Rect::new( area.x, center_y, area.width, 1 );
  Paragraph::new( line ).render( centered_area, buf );
}

pub fn on_key_down_for_table_state( key: &KeyCode, state: &mut TableState, count: usize ) -> bool {
  match key {
    KeyCode::Down => {
      let next_idx = ( state.selected().unwrap_or( 0 ) + 1 ).min( count.saturating_sub( 1 ) );
      state.select( Some( next_idx ) );
      true
    }
    KeyCode::Up => {
      let next_idx = state.selected().unwrap_or( 0 ).saturating_sub( 1 );
      state.select( Some( next_idx ) );
      true
    }
    _ => false
  }
}