use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Rect };
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{ Block, Cell, Padding, Row, Widget, Table };

use super::{ Context, IsScene, SceneEvent };

#[derive( Default )]
pub struct InfoScene;

impl IsScene for InfoScene {
  fn on_key_down( &mut self, ctx: &Context, key: KeyCode ) -> SceneEvent {
    match key {
      KeyCode::Esc | KeyCode::Char( 'q' ) => SceneEvent::Exit,
      KeyCode::Char( 'c' ) => {
        if let Some( device ) = ctx.selected_device() {
          SceneEvent::Connect( *device.info().broadcast_address() )
        } else {
          SceneEvent::Handled
        }
      }
      _ => SceneEvent::NotHandled
    }
  }

  fn render( &mut self, ctx: &Context, area: Rect, buf: &mut Buffer ) {
    if let Some( device ) = ctx.selected_device() {
      let block = Block::bordered()
        .title( Line::raw( format!( " Device: {} ", device.info().broadcast_address() ) ).centered() )
        .padding( Padding::uniform( 1 ) );

      let constraints = [ Constraint::Length( 25 ), Constraint::Fill( 1 ) ];

      let mut rows = Vec::with_capacity( 50 );
      rows.extend([
        Row::new([ Cell::new( "Intrinsics" ).style( Style::new().bold() ) ]),
        Row::new([ " IP address:".to_owned(), device.ip().to_string() ]),
        Row::new([ " MAC address:".to_owned(), device.info().mac_address().to_string() ]),
        Row::new([ " Version:".to_owned(), format!( "Hardware: {}; Software: {};", device.info().version().hardware, device.info().version().software ) ]),
        Row::new([ " Point buffer capacity:".to_owned(), device.info().buffer_capacity().to_string() ]),
        Row::new([ " Max points per second:".to_owned(), device.info().max_points_per_second().to_string() ])
      ]);

      //
      rows.push( Row::new([ Cell::new( "State" ).style( Style::new().bold() ) ]).top_margin( 1 ) );

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
  }
}