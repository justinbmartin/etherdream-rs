use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::style::{ Color, Style };
use ratatui::widgets::{ Paragraph, Widget };

pub const INFO_TEXT_STYLE: Style = Style::new().fg( Color::Rgb( 150, 150, 150 ) );
pub const HIGHLIGHT_COLOR: Color = Color::Rgb( 232, 195, 0 );
pub const HIGHLIGHT_BORDER_STYLE: Style = Style::new().fg( HIGHLIGHT_COLOR );
pub const HIGHLIGHT_ROW_SELECTED_STYLE: Style = Style::new().bg( HIGHLIGHT_COLOR ).bold().fg( Color::Black );
pub const HIGHLIGHT_TEXT_STYLE: Style = Style::new().fg( HIGHLIGHT_COLOR ).bold();
pub const TABLE_HEADER_STYLE: Style = Style::new().bold();

pub fn layout( area: Rect, buf: &mut Buffer, footer_msg: &str ) -> Rect {
  let layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
  let [ body, footer ] = area.layout( &layout );

  Paragraph::new( footer_msg ).style( INFO_TEXT_STYLE ).centered().render( footer, buf );
  body
}