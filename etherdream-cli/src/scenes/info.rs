use super::{ IsScene, SceneData, SceneEvent };

use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{ Block, Paragraph, Widget };

pub struct InfoScene {
  data: SceneData
}

impl InfoScene {
  pub fn new( data: SceneData ) -> Self {
    Self{ data }
  }
}

impl IsScene for InfoScene {
  fn on_key_press( &'_ mut self, key: KeyCode ) -> SceneEvent<'_> {
    match key {
      KeyCode::Esc | KeyCode::Char( 'q' ) => SceneEvent::Exit,
      _ => SceneEvent::NotHandled
    }
  }

  fn render( &mut self, area: Rect, buf: &mut Buffer ) {
    if let Some( device ) = self.data.selected_device() {
      let block = Block::bordered().title( Line::raw( format!( " Device: {} ", device.info().address() ) ).centered() );

      Paragraph::new( format!( "MAC Address: {}", device.info().mac_address() ) )
        .centered()
        .block( block )
        .render( area, buf )
    }
  }
}