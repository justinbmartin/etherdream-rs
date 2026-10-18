use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::scene;
use crate::ui::{ Action, State };

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

  fn on_draw( &mut self, _area: Rect, _buf: &mut Buffer, _state: &scene::Context<State> ) {
    // ...
  }
}

