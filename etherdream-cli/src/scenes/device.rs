use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::style::{ Color, Style };
use ratatui::text::Span;
use ratatui::widgets::{ Block, Padding, Paragraph, Row, Widget, Table };
use ratatui_textarea::TextArea;

use crate::device::Device;
use crate::scene::{ Scene, SceneContext, SceneEvent };

const CONNECT_BUTTON_INDEX: usize = 1;
const KEY_WIDTH: u16 = 25;
const PORT_INPUT_INDEX: usize = 0;

pub struct DeviceScene<'a> {
  state: etherdream::State,

  // Connect pane properties
  connect_input_selected: usize,
  connect_port_input: TextArea<'a>
}

impl<'a> Default for DeviceScene<'a> {
  fn default() -> Self {
    let mut connect_port_input = TextArea::default();
    connect_port_input.set_cursor_line_style( Style::default() );
    connect_port_input.set_placeholder_text( etherdream::protocol::CLIENT_PORT.to_string() );

    Self{
      connect_input_selected: CONNECT_BUTTON_INDEX,
      connect_port_input,
      state: etherdream::State::default()
    }
  }
}

impl<'a> Scene for DeviceScene<'a> {
  fn on_scene_enter( &mut self ) {
    self.connect_input_selected = CONNECT_BUTTON_INDEX;
  }

  fn on_key_down( &mut self, ctx: &SceneContext, key: KeyEvent ) -> SceneEvent {
    if let Some( device ) = ctx.selected_device() {
      if device.is_connected() {
        match key.code {
          KeyCode::Enter => return SceneEvent::Play( device.id() ),
          KeyCode::Esc | KeyCode::Char( 'q' ) => return SceneEvent::Exit,
          _ => {}
        }
      } else {
        match key.code {
          KeyCode::Up => {
            self.connect_input_selected = PORT_INPUT_INDEX;
            return SceneEvent::Handled;
          },
          KeyCode::Down => {
            self.connect_input_selected = CONNECT_BUTTON_INDEX;
            return SceneEvent::Handled;
          },
          KeyCode::Enter => {
            if self.connect_input_selected == CONNECT_BUTTON_INDEX && let Some( device ) = ctx.selected_device() {
              return SceneEvent::Connect( device.id() );
            }
          },
          KeyCode::Esc | KeyCode::Char( 'q' ) => return SceneEvent::Exit,
          _ => {
            if self.connect_input_selected == 0 && self.connect_port_input.input( key ) {
              let _is_valid = validate_port( &mut self.connect_port_input );
            }
          }
        };
      }
    }

    SceneEvent::NotHandled
  }

  fn render( &mut self, ctx: &SceneContext, area: Rect, buf: &mut Buffer ) {
    if let Some( device ) = ctx.selected_device() {
      let layout = Layout::vertical([ Constraint::Length( 3 ), Constraint::Fill( 1 ) ]);
      let [ header, body ] = area.layout( &layout );

      // Render the header
      Paragraph::new( format!( " Device: {} ", device.info().ip() ) ).render( header, buf );

      //
      let [ test_area, info_area ] = body.layout( &Layout::horizontal([
        Constraint::Fill( 1 ),
        Constraint::Length( 60 )
      ]) );

      // Render the test pane
      let test_block = Block::bordered().title( " Connect " );
      let test_inner_area = test_block.inner( test_area );
      test_block.render( test_area, buf );

      if device.is_connected() {
        self.render_generator_pane( test_inner_area, buf );
      } else {
        self.render_connect_pane( test_inner_area, buf );
      }

      // Render the info pane
      self.render_info( &device, info_area, buf );
    }
  }
}

impl<'a> DeviceScene<'a> {
  // UI to render the Etherdream device intrinsic and run-time properties
  fn render_info( &mut self, device: &Device, area: Rect, buf: &mut Buffer ) {
    let [ intrinsics_area, state_area ] = area.layout( &Layout::vertical([
      Constraint::Length( 10 ), Constraint::Fill( 1 ),
    ]) );

    // Render intrinsics
    let intrinsics_block = Block::bordered()
      .title( " Intrinsics " )
      .padding( Padding::uniform( 1 ) );

    let intrinsic_rows = [
      Row::new([ "IP address:".to_owned(), device.info().ip().to_string() ]),
      Row::new([ "MAC address:".to_owned(), device.info().mac_address().to_string() ]),
      Row::new([ "Hardware version:".to_owned(), device.info().version().hardware.to_string() ]),
      Row::new([ "Software version:".to_owned(), device.info().version().software.to_string() ]),
      Row::new([ "Point buffer capacity:".to_owned(), device.info().buffer_capacity().to_string() ]),
      Row::new([ "Max points per second:".to_owned(), device.info().max_points_per_second().to_string() ])
    ];

    Table::new( intrinsic_rows, [ Constraint::Length( KEY_WIDTH ), Constraint::Fill( 1 ) ])
      .block( intrinsics_block )
      .render( intrinsics_area, buf );


    // Render state
    let state_block = Block::bordered().title( " State " ).padding( Padding::horizontal( 1 ) );
    let mut rows = Vec::with_capacity( 50 );
    rows.push( Row::new([ "Connected:", if device.is_connected() { "Yes" } else { "No" } ]) );

    if let Some( generator ) = device.generator() {
      generator.clone_state_into( &mut self.state );
      rows.push( Row::new([ "Generator:", "Demo" ]) );
      rows.extend([
        Row::new([ "Points buffered:".to_owned(), self.state.points_buffered().to_string() ]),
        Row::new([ "Points per second:".to_owned(), self.state.points_per_second().to_string() ])
      ]);
    } else {
      rows.push( Row::new([ "Generator:", "None" ]) );
    }

    Table::new( rows, [ Constraint::Length( KEY_WIDTH ), Constraint::Fill( 1 ) ])
      .block( state_block )
      .render( state_area, buf );
  }

  // UI to render a form to connect to an Etherdream device
  fn render_connect_pane( &mut self, area: Rect, buf: &mut Buffer ) {
    let centered_area = area.centered_horizontally( Constraint::Length( 50 ) );

    let [ port_area, connect_area, _ ] = centered_area.layout( &Layout::vertical([
      Constraint::Length( 3 ),
      Constraint::Length( 3 ),
      Constraint::Fill( 1 )
    ]));

    //
    let button_highlight_style = Style::default().fg( Color::Green );

    // Port input
    let style = if self.connect_input_selected == 0 { button_highlight_style } else { Style::default() };
    let port_block = Block::bordered().title( " Port " ).border_style( style );
    self.connect_port_input.set_block( port_block );

    Widget::render( &self.connect_port_input, port_area, buf );

    // Connect button
    let style = if self.connect_input_selected == CONNECT_BUTTON_INDEX { button_highlight_style } else { Style::default() };
    let connect_block = Block::bordered().border_style( style );
    let connect_btn = Paragraph::new( Span::styled( "<C>onnect", Style::default().bold() ) ).centered().block( connect_block );
    Widget::render( connect_btn, connect_area, buf );
  }

  // UI to start a generator on a connected Etherdream device
  fn render_generator_pane( &mut self, area: Rect, buf: &mut Buffer ) {
    Paragraph::new( ">> Generate <<" ).render( area, buf );
  }
}


fn validate_port( port: &mut TextArea ) -> bool {
  if let Err( _ ) = port.lines()[0].parse::<u16>() {
    port.set_style( Style::default().fg( Color::LightRed ) );
    false
  } else {
    port.set_style( Style::default().fg( Color::LightGreen ) );
    true
  }
}