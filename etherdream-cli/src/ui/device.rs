use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Alignment, Constraint, Layout, Rect };
use ratatui::text::Line;
use ratatui::widgets::{ Block, Cell, Padding, Paragraph, Row, StatefulWidget, Table, TableState, Widget };

use crate::device::Device;
use crate::scene;
use crate::ui::{ Action, State };
use super::common;

const TABLE_KEY_WIDTH: u16 = 23;

const FOOTER_CONNECTED_MSG: &str = "Use 'd' to disconnect, 'Esc' to go back.";
const FOOTER_DISCONNECTED_MSG: &str = "Use 'Esc' to go back.";

#[derive( Default )]
pub struct DeviceScene;

impl scene::Scene<State> for DeviceScene {
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
        ctx.invoke( Action::DeselectDevice );
        true
      }
      _ => {
        false
      }
    }
  }

  fn on_draw( &mut self, area: Rect, buf: &mut Buffer, ctx: &scene::Context<State> ) {
    if let Some( device ) = ctx.state().get_current_device() {
      let footer_msg = if device.is_connected() { FOOTER_CONNECTED_MSG } else { FOOTER_DISCONNECTED_MSG };
      let body_area = common::layout( area, buf, footer_msg );

      let layout = Layout::horizontal([ Constraint::Length( 52 ), Constraint::Fill( 1 ) ]);
      let [ left_area, right_area ] = layout.areas( body_area );

      // Render left pane
      let properties_block = Block::bordered()
        .border_style( common::HIGHLIGHT_BORDER_STYLE )
        .title( Line::styled( " Properties ", common::HIGHLIGHT_TEXT_STYLE ) )
        .padding( Padding::uniform( 1 ) );

      render_info( device, properties_block.inner( left_area ), buf );
      properties_block.render( left_area, buf );

      // Render right pane
      let action_block = Block::bordered()
        .border_style( common::HIGHLIGHT_BORDER_STYLE )
        .padding( Padding::new( 1, 1, 0, 0 ) );

      let action_block =
        if device.is_connected() {
          let action_block = action_block.title( Line::raw( " Generators " ).centered().style( common::HIGHLIGHT_TEXT_STYLE ) );
          render_generator_pane( action_block.inner( right_area ), buf );
          action_block
        } else {
          let inner_area = action_block.inner( right_area );
          let center_y = inner_area.y + ( inner_area.height / 2 );
          let text_area = Rect::new( inner_area.x, center_y, inner_area.width, 1 );
          let line = Line::styled( " Press 'c' to connect. ", common::HIGHLIGHT_ROW_SELECTED_STYLE ).alignment( Alignment::Center );
          Paragraph::new( line ).render( text_area, buf );
          action_block
        };

      action_block.render( right_area, buf );
    }
  }
}

// UI to render the Etherdream device intrinsic and run-time properties
fn render_info( device: &Device, area: Rect, buf: &mut Buffer ) {
  let [ intrinsics_area, state_area ] = area.layout( &Layout::vertical([
    Constraint::Length( 8 ), Constraint::Fill( 1 ),
  ]) );

  // Render intrinsics
  let intrinsics_block = Block::new().style( common::HIGHLIGHT_BORDER_STYLE );

  let intrinsic_rows = [
    Row::new([ "IP address:".to_owned(), device.info().ip().to_string() ]),
    Row::new([ "MAC address:".to_owned(), device.info().mac_address().to_string() ]),
    Row::new([ "Hardware version:".to_owned(), device.info().version().hardware.to_string() ]),
    Row::new([ "Software version:".to_owned(), device.info().version().software.to_string() ]),
    Row::new([ "Point buffer capacity:".to_owned(), device.info().buffer_capacity().to_string() ]),
    Row::new([ "Max points per second:".to_owned(), device.info().max_points_per_second().to_string() ])
  ];

  let table = Table::new( intrinsic_rows, [ Constraint::Length( TABLE_KEY_WIDTH ), Constraint::Fill( 1 ) ])
    .block( intrinsics_block );

  Widget::render( table, intrinsics_area, buf );

  // Render state
  let state_block = Block::new()
    .style( common::HIGHLIGHT_BORDER_STYLE )
    .title( Line::styled( "STATE:", common::HIGHLIGHT_TEXT_STYLE ) );

  let mut rows = Vec::with_capacity( 50 );
  rows.push( Row::new([ "Connected:", if device.is_connected() { "Yes" } else { "No" } ]) );

  /*
  if let Some( generator ) = device.generator() {
    generator.clone_state_into( state );

    rows.extend([
      Row::new([ "Generator:", "Demo" ]),
      Row::new([ "Points buffered:".to_owned(), state.points_buffered().to_string() ]),
      Row::new([ "Points per second:".to_owned(), state.points_per_second().to_string() ])
    ]);
  } else {
    rows.push( Row::new([ "Generator:", "None" ]) );
  }
  */

  let table = Table::new( rows, [ Constraint::Length( TABLE_KEY_WIDTH ), Constraint::Fill( 1 ) ])
    .block( state_block );

  Widget::render( table, state_area, buf )
}

fn render_generator_pane( area: Rect, buf: &mut Buffer ) {
  let table_rows = vec![ Row::new([ Cell::new( "Demo" ), Cell::new( "" ) ]).style( common::HIGHLIGHT_ROW_SELECTED_STYLE ) ];

  const TABLE_HEADERS: &[&str] = &[ "NAME", "STATUS" ];

  const CONSTRAINTS: [Constraint; 2] = [ Constraint::Min( 30 ), Constraint::Fill( 1 ) ];

  let table = Table::new( table_rows, CONSTRAINTS )
    .header( Row::new( TABLE_HEADERS.iter().cloned() ).style( common::TABLE_HEADER_STYLE ) );

  let mut state = TableState::new().with_selected( Some( 0 ) );
  StatefulWidget::render( table, area, buf, &mut state );
}

/*
// - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Generate List Scene

struct GeneratorListScene {
  generators: Vec<String>,
  shared: SharedData,
  state: TableState
}

impl GeneratorListScene {
  fn new( shared: SharedData ) -> Self {
    Self{
      generators: vec![ "Demo".to_owned() ],
      shared,
      state: TableState::new().with_selected( Some( 0 ) )
    }
  }
}

impl scene::Scene<app::Event> for GeneratorListScene {
  fn on_key_down( &mut self, key: KeyEvent ) -> bool {
    match key.code {
      KeyCode::Up => {
        self.state.select( Some( self.state.selected().unwrap().saturating_sub( 1 ) ) );
        return true;
      },
      KeyCode::Down => {
        self.state.select( Some( self.state.selected().unwrap().saturating_add( 1 ) % self.generators.len() ) );
        return true;
      },
      KeyCode::Enter => {
        if let Some( _device ) = self.shared.selected_device() {
          //return SceneEvent::Play( device.id() );
          return true
        }
      },
      _ => {}
    }

    false
  }

  fn on_update( &mut self ) -> scene::Event<app::Event> {
    scene::Event::NoChange
  }

  fn on_render( &mut self, area: Rect, buf: &mut Buffer ) {
    let constraints = [ Constraint::Fill( 1 ) ];
    let selected = self.state.selected().unwrap_or( 0 );

    let rows = self.generators
      .iter()
      .enumerate()
      .map(| ( i, name ) | {
        let theme = if selected == i { HIGHLIGHT_STYLE } else { Style::new() };
        Row::new([ Cell::new( name.to_owned() ).style( theme ) ])
      });

    let table = Table::new( rows, constraints )
      .header( Row::new( vec![ "Generator Name" ] ).style( Style::new().bold() ) )
      .highlight_spacing( ratatui::widgets::HighlightSpacing::Always )
      .highlight_symbol( "> " );

    table.render( area, buf );
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Generate Scene

struct GeneratorScene {}

impl Default for GeneratorScene {
  fn default() -> Self {
    Self{}
  }
}

impl scene::Scene<app::Event> for GeneratorScene {
  fn on_key_down( &mut self, _key: KeyEvent ) -> bool {
    /*
    match key.code {
      KeyCode::Enter => {
        if let Some( device ) = ctx.selected_device() {
          return scene::Event::Play( device.id() );
        }
      },
      _ => {}
    }
    */

    false
  }

  fn on_update( &mut self ) -> scene::Event<app::Event> { scene::Event::NoChange }

  fn on_render( &mut self, _area: Rect, _buf: &mut Buffer ) { }
}
*/