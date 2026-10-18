use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::widgets::{ Cell, Row, StatefulWidget, Table, TableState, Widget };

use crate::scene;
use crate::ui::{ Action, State };
use super::common;

const CONNECT_MSG: &str = "Press 'c' to connect.";
const FOOTER_CONNECTED_MSG: &str = "Use 'p' to play, 'd' to disconnect, 'Esc' to go back.";
const FOOTER_DISCONNECTED_MSG: &str = "Use 'Esc' to go back.";

pub struct DeviceScene{
  generators: TableState
}

impl DeviceScene {
  pub fn new() -> Self {
    Self{
      generators: TableState::new().with_selected( Some( 0 ) )
    }
  }
}

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
      KeyCode::Char( 'p' ) => {
        ctx.invoke( Action::Play( "demo" ) );
        true
      }
      KeyCode::Char( 'q' ) => {
        ctx.invoke( Action::Back );
        true
      }
      KeyCode::Enter => {
        ctx.invoke( Action::ConfigureGenerator( "" ) );
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

      let layout = Layout::horizontal([ Constraint::Length( 54 ), Constraint::Fill( 1 ) ]);
      let [ left_area, right_area ] = layout.areas( body_area );

      // Render left pane
      common::draw_device_panel( device, left_area, buf );

      // Render right pane
      if device.is_connected() {
        let block = common::make_block( Some( " Generators " ) );
        let inner_area = block.inner( right_area );

        Widget::render( block, right_area, buf );
        render_generator_pane( &mut self.generators, inner_area, buf );
      } else {
        let block = common::make_block( None );
        let inner_area = block.inner( right_area );

        Widget::render( block, right_area, buf );
        common::draw_centered_line( CONNECT_MSG, common::HIGHLIGHT_ROW_SELECTED_STYLE, inner_area, buf );
      }
    } else {
      // something went wrong...
    }
  }
}

fn render_generator_pane( state: &mut TableState, area: Rect, buf: &mut Buffer ) {
  const CONSTRAINTS: [Constraint; 2] = [ Constraint::Min( 30 ), Constraint::Fill( 1 ) ];

  let rows = vec![ Row::new([ Cell::new( "Demo" ), Cell::new( "" ) ])
    .style( common::HIGHLIGHT_ROW_SELECTED_STYLE ) ];

  let table = Table::new( rows, CONSTRAINTS )
    .header( Row::new([ "NAME", "STATUS" ]).style( common::TABLE_HEADER_STYLE ) );

  StatefulWidget::render( table, area, buf, state );
}