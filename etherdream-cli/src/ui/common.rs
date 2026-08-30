use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::widgets::{ Paragraph, Widget };

pub fn layout( area: Rect, buf: &mut Buffer, footer_msg: &str ) -> Rect {
  let layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
  let [ body, footer ] = area.layout( &layout );

  Paragraph::new( footer_msg ).centered().render( footer, buf );
  body
}