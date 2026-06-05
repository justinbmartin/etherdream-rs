use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::style::{ Color, Style };
use ratatui::text::Line;
use ratatui::widgets::{ Block, Cell, Row, Widget, Table, Tabs };
use ratatui_textarea::{ Input, Key, TextArea };

use super::{ Context, Device, IsScene, SceneEvent };

#[derive( Default )]
pub struct InfoScene {
  selected: usize,
  port_input: Vec<String>
}

impl IsScene for InfoScene {
  fn on_key_down( &mut self, ctx: &Context, key: KeyCode ) -> SceneEvent {
    match key {
      KeyCode::Left => {
        self.selected = self.selected.saturating_sub( 1 );
        SceneEvent::Handled
      }
      KeyCode::Right => {
        self.selected = self.selected.saturating_add( 1 ).min( 1 );
        SceneEvent::Handled
      }
      KeyCode::Esc | KeyCode::Char( 'q' ) => SceneEvent::Exit,
      KeyCode::Char( 'c' ) => {
        if let Some( device ) = ctx.selected_device() {
          SceneEvent::Connect( device.id() )
        } else {
          SceneEvent::Handled
        }
      }
      _ => SceneEvent::NotHandled
    }
  }

  fn render( &mut self, ctx: &Context, area: Rect, buf: &mut Buffer ) {
    if let Some( device ) = ctx.selected_device() {
      let layout = Layout::vertical([ Constraint::Length( 3 ), Constraint::Fill( 1 ) ]);
      let [ menu, content ] = area.layout( &layout );

      // Render menu
      self.render_menu( menu, buf, &device );

      // Render Info
      if self.selected == 0 {
        self.render_info( content, buf, &device )
      } else {
        self.render_connect( content, buf, &device )
      }
    }
  }
}

impl InfoScene {
  fn render_menu( &self, area: Rect, buf: &mut Buffer, device: &Device ) {
    let block = Block::bordered()
      .title( Line::raw( format!( " Device: {} ", device.info().ip() ) ).centered() );

    let tabs = Tabs::new( vec![ "Info", "Connect" ])
      .block( block )
      .style( Color::White )
      .highlight_style( Style::default().magenta().on_black().bold() )
      .select( self.selected )
      .divider( "|" )
      .padding( " ", " " );

    Widget::render( tabs, area, buf );
  }

  fn render_info( &self, area: Rect, buf: &mut Buffer, device: &Device ) {
    let block = Block::bordered();

    let constraints = [ Constraint::Length( 25 ), Constraint::Fill( 1 ) ];

    let mut rows = Vec::with_capacity( 50 );
    rows.extend([
      Row::new([ Cell::new( "Intrinsics" ).style( Style::new().bold() ) ]),
      Row::new([ " IP address:".to_owned(), device.info().ip().to_string() ]),
      Row::new([ " MAC address:".to_owned(), device.info().mac_address().to_string() ]),
      Row::new([ " Version:".to_owned(), format!( "Hardware: {}; Software: {};", device.info().version().hardware, device.info().version().software ) ]),
      Row::new([ " Point buffer capacity:".to_owned(), device.info().buffer_capacity().to_string() ]),
      Row::new([ " Max points per second:".to_owned(), device.info().max_points_per_second().to_string() ])
    ]);

    //
    rows.push( Row::new([ Cell::new( "State" ).style( Style::new().bold() ) ]) );

    if let Some( generator ) = device.generator() {
      rows.extend([
        Row::new([ " Connected:", "Yes" ]),
        Row::new([ " Running:".to_owned(), generator.is_running().to_string() ])
      ]);
    } else {
      rows.push( Row::new([ " Connected:", "No" ]) );
    }

    let table = Table::new( rows, constraints ).block( block );
    Widget::render( table, area, buf );
  }

  fn render_connect( &mut self, area: Rect, buf: &mut Buffer, _device: &Device ) {
    let block = Block::bordered();

    let mut textarea = TextArea::new( self.port_input );
    textarea.set_cursor_line_style( Style::default() );
    textarea.set_placeholder_text( etherdream::protocol::CLIENT_PORT.to_string() );

    block.render( area, buf );
  }
}