use std::collections::BTreeMap;

use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::{ Alignment, Constraint, Layout, Rect };
use ratatui::style::{ Color, Style };
use ratatui::text::Line;
use ratatui::widgets::{ self, Block, Padding, Paragraph, Row, StatefulWidget, Widget };

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

pub fn on_key_down_for_table_state( key: &KeyCode, state: &mut widgets::TableState, count: usize ) -> bool {
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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Table

pub struct TableState<K: Ord,V> {
  items: BTreeMap<K,V>,
  state: widgets::TableState
}

impl<K,V> TableState<K,V>
  where K: Ord
{
  pub fn new() -> Self {
    Self{ items: BTreeMap::new(), state: widgets::TableState::new().with_selected( 0 ) }
  }

  pub fn clear( &mut self ) {
    self.items.clear();
    self.state.select( Some( 0 ) );
  }

  pub fn is_empty( &self ) -> bool { self.items.is_empty() }

  pub fn len( &self ) -> usize { self.items.len() }

  pub fn selected( &self ) -> Option<&K> {
    self.state.selected().and_then(| idx | self.items.keys().nth( idx ) )
  }

  pub fn insert( &mut self, key: K, value: V ) -> Option<V> {
    self.items.insert( key, value )
  }

  pub fn handle_key_event( &mut self, key: &KeyCode ) -> bool {
    match key {
      KeyCode::Down => {
        let next_idx = ( self.state.selected().unwrap_or( 0 ) + 1 ).min( self.items.len().saturating_sub( 1 ) );
        self.state.select( Some( next_idx ) );
        true
      }
      KeyCode::Up => {
        let next_idx = self.state.selected().unwrap_or( 0 ).saturating_sub( 1 );
        self.state.select( Some( next_idx ) );
        true
      }
      _ => false
    }
  }

  pub fn render<'a, 'b, C, F: FnMut( ( &'b K, &'b V ), bool ) -> Row<'b>>(
    &'a mut self,
    widths: C,
    area: Rect,
    buf: &mut Buffer,
    mut callback: F
  )
    where 'a: 'b,
          C: IntoIterator, <C as IntoIterator>::Item: Into<Constraint>
  {
    let rows = self.items.iter().map(|( k, v )|{
      let is_selected = self.selected() == Some( k );
      callback( ( k, v ), is_selected )
    });
    
    let table = widgets::Table::new( rows, widths )
      .header( Row::new([ "IP", "MAC", "STATUS" ]).style( TABLE_HEADER_STYLE ) );

    StatefulWidget::render( table, area, buf, &mut self.state );
  }
}