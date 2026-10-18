use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };

use crate::scene;
use crate::ui::{ Action, State };
use super::common;

pub struct GeneratorScene;

impl GeneratorScene {
  pub fn new() -> Self {
    Self{}
  }
}

impl scene::Scene<State> for GeneratorScene {
  fn on_key_down( &mut self, key: KeyEvent, ctx: &mut scene::Context<State> ) -> bool {
    match key.code {
      KeyCode::Char( 'q' ) => {
        ctx.invoke( Action::Back );
        true
      }
      _ => {
        false
      }
    }
  }

  fn on_update( &mut self, _ctx: &mut scene::Context<State> ) {
    // ...
  }

  fn on_draw( &mut self, area: Rect, buf: &mut Buffer, ctx: &scene::Context<State> ) {
    let body_area = common::layout( area, buf, "Use 'Esc' to go back" );

    if let Some( device ) = ctx.state().get_current_device() {
      let layout = Layout::horizontal([ Constraint::Length( 54 ), Constraint::Fill( 1 ) ]);
      let [ left_area, _right_area ] = layout.areas( body_area );

      // Render left pane
      common::draw_device_panel( device, left_area, buf );

      // Render right pane
      // ...
    } else {
      // something went wrong...
    }
  }
}

