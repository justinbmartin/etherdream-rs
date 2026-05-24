use std::net::SocketAddr;

use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
//use ratatui::style::{ palette::tailwind::SLATE, Style };
use ratatui::text::Line;
use ratatui::widgets::{ Block, Paragraph, Widget };

use super::{ Context, IsScene, SceneEvent };

//const SELECTED: Style = Style::new().bg( SLATE.c800 );

#[derive( Default )]
pub struct InfoScene;

impl IsScene for InfoScene {
  fn on_key_down( &mut self, ctx: &mut Context, key: KeyCode ) -> SceneEvent {
    match key {
      KeyCode::Esc | KeyCode::Char( 'q' ) => SceneEvent::Exit,
      KeyCode::Char( 'c' ) => {
        if let Some( device ) = ctx.selected_device() {
          SceneEvent::Connect( *device.info().address() )
        } else {
          SceneEvent::Handled
        }
      }
      _ => SceneEvent::NotHandled
    }
  }

  fn render( &mut self, ctx: &Context, area: Rect, buf: &mut Buffer ) {
    if let Some( device ) = ctx.selected_device() {

      // Info
      let block = Block::bordered().title( Line::raw( format!( " Device: {} ", device.info().address() ) ).centered() );
      Paragraph::new( format!( "MAC Address: {}", device.info().mac_address() ) )
        .centered()
        .block( block )
        .render( area, buf )
    }
  }
}