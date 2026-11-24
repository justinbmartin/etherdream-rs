use std::collections::BTreeMap;

use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::{ Alignment, Constraint, Layout, Rect };
use ratatui::style::{ Color, Style };
use ratatui::text::Line;
use ratatui::widgets::{ self, Block, Padding, Paragraph, Row, StatefulWidget, TableState, Widget };

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

// DEPRECATE
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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Simple Table

pub struct SimpleTableTheme {
  header_style: Style,
  row_style: Style,
  row_highlight_style: Style
}

impl SimpleTableTheme {
  pub fn new() -> Self {
    Self{
      header_style: TABLE_HEADER_STYLE,
      row_style: HIGHLIGHT_TEXT_STYLE,
      row_highlight_style: HIGHLIGHT_ROW_SELECTED_STYLE
    }
  }
}

pub struct SimpleTable<K,V, const N: usize>
  where K: Ord
{
  headers: [String; N],
  items: BTreeMap<K,V>,
  state: TableState,
  theme: SimpleTableTheme,
  widths: [Constraint; N]
}

impl<K,V,const N: usize> SimpleTable<K,V,N>
  where K: Ord
{
  /// Creates a new `SimpleTable`.
  pub fn new( headers: [impl Into<String>; N], widths: [Constraint; N] ) -> Self {
    Self{
      headers: headers.map(| s | s.into() ),
      items: BTreeMap::new(),
      state: TableState::new().with_selected( 0 ),
      theme: SimpleTableTheme::new(),
      widths
    }
  }

  /// Clears the table.
  pub fn clear( &mut self ) {
    self.items.clear();
    self.state.select( Some( 0 ) );
  }

  /// Returns true if the table is empty.
  pub fn is_empty( &self ) -> bool { self.items.is_empty() }

  /// Returns the count of items in the table.
  pub fn len( &self ) -> usize { self.items.len() }

  /// Returns the selected key for the table. Will return `None` if the table
  /// is empty.
  pub fn selected( &self ) -> Option<&K> {
    self.state.selected().and_then(| idx | self.items.keys().nth( idx ) )
  }

  /// Adds a new item to the table.
  pub fn insert( &mut self, key: K, value: V ) -> Option<V> {
    self.items.insert( key, value )
  }

  /// Selects the next or previous table row depending on `KeyCode`. Will
  /// return `true` if the event was handled. Will return `false` otherwise.
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

  /// Renders the table using the provided `area` and `buffer`. Each item in
  /// table will be passed into `callback` for handling.
  pub fn render<'a, 'b, F>( &'a mut self, area: Rect, buf: &mut Buffer, callback: F )
    where 'a: 'b,
          F: Fn( ( &'b K, &'b V ), bool ) -> [&str; N]
  {
    let rows = self.items.iter().map(|( k, v )|{
      let row_selected = self.selected() == Some( k );
      let row = Row::new( callback( ( k, v ), row_selected ) );

      if row_selected {
        row.style( self.theme.row_highlight_style )
      } else {
        row.style( self.theme.row_style )
      }
    });

    let table = widgets::Table::new( rows, &self.widths )
      .header( Row::new( self.headers.iter().map(| s | s.as_str() ) ).style( self.theme.header_style ) );

    StatefulWidget::render( table, area, buf, &mut self.state );
  }
}