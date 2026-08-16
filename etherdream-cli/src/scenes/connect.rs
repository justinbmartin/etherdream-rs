use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{ Color, Style };
use ratatui_textarea::TextArea;

use crate::app;
use crate::device;
use crate::scene;

const INPUT_CONNECT_BUTTON: usize = 1;
const INPUT_PORT: usize = 0;

// Scene that renders a form to connect to an Etherdream device.
pub struct ConnectScene<'a> {
  connect: bool,
  device: device::ScopedDevice,
  input_selected: usize,
  port_input: TextArea<'a>
}

impl<'a> ConnectScene<'a> {
  pub fn new( device: device::ScopedDevice ) -> Self {
    let mut port_input = TextArea::default();
    port_input.set_cursor_line_style( Style::default() );
    port_input.set_placeholder_text( etherdream::protocol::CLIENT_PORT.to_string() );

    Self{
      connect: false,
      device,
      input_selected: INPUT_CONNECT_BUTTON,
      port_input,
    }
  }
}

impl<'a> scene::Scene<app::App> for ConnectScene<'a> {
  fn on_enter( &mut self ) {
    self.input_selected = INPUT_CONNECT_BUTTON;
  }

  fn on_key_down( &mut self, key: KeyEvent ) -> bool {
    match key.code {
      KeyCode::Up => {
        self.input_selected = INPUT_PORT;
        return true;
      },
      KeyCode::Down => {
        self.input_selected = INPUT_CONNECT_BUTTON;
        return true;
      },
      KeyCode::Enter => {
        if self.input_selected == INPUT_CONNECT_BUTTON  {
          self.connect = true;
          true
        } else {
          false
        }
      },
      //KeyCode::Esc | KeyCode::Char( 'q' ) => return sceneEvent::Exit,
      _ => {
        if self.input_selected == 0 && self.port_input.input( key ) {
          let _is_valid = validate_port( &mut self.port_input );
          return true;
        }

        return false;
      }
    };

    false
  }

  fn on_update( &mut self, ctx: &mut scene::UpdateContext<app::App> ) {
    if self.connect {
      self.connect = false;
      ctx.invoke( app::Action::Connect( self.port_input.lines()[0].parse::<u16>().unwrap() ) );
    }
  }

  fn on_draw( &mut self, area: Rect, buf: &mut Buffer ) {

  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Helpers

// Validates that the port input is a `u16`.`
fn validate_port( port: &mut TextArea ) -> bool {
  if let Err( _ ) = port.lines()[0].parse::<u16>() {
    port.set_style( Style::default().fg( Color::LightRed ) );
    false
  } else {
    port.set_style( Style::default().fg( Color::LightGreen ) );
    true
  }
}