use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::style::{ Color, Style };
use ratatui::text::Span;
use ratatui::widgets::{ Block, Cell, Paragraph, Row, Widget, Table };
use ratatui_textarea::TextArea;

use crate::device::Device;
use crate::scene::{ Scene, SceneContext, SceneEvent };

const CONNECT_BUTTON_INDEX: usize = 1;

pub struct DeviceScene<'a> {
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
      connect_port_input
    }
  }
}

impl<'a> Scene for DeviceScene<'a> {
  fn on_scene_enter( &mut self ) {
    self.connect_input_selected = CONNECT_BUTTON_INDEX;
  }

  fn on_key_down( &mut self, ctx: &SceneContext, key: KeyCode ) -> SceneEvent {
    if let Some( device ) = ctx.selected_device() {
      if device.is_connected() {

      } else {
        match key {
          KeyCode::Up => {
            self.connect_input_selected = 0;
            return SceneEvent::Handled;
          },
          KeyCode::Down => {
            self.connect_input_selected = 1;
            return SceneEvent::Handled;
          },
          KeyCode::Enter => {
            if self.connect_input_selected == CONNECT_BUTTON_INDEX && let Some( device ) = ctx.selected_device() {
              return SceneEvent::Connect( device.id() );
            }
          },
          KeyCode::Esc | KeyCode::Char( 'q' ) => {
            return SceneEvent::Exit;
          },
          _ => {}
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

      // Render the info pane
      let [ info_area, test_area ] = body.layout( &Layout::horizontal([ Constraint::Length( 60 ), Constraint::Fill( 1 ) ]) );
      self.render_info( &device, info_area, buf );

      // Render the test pane
      let test_block = Block::bordered().title( " Test " );
      let test_inner_area = test_block.inner( test_area );
      test_block.render( test_area, buf );

      if device.is_connected() {
        // todo
      } else {
        self.render_connect_pane( test_inner_area, buf );
      }
    }
  }
}

impl<'a> DeviceScene<'a> {
  fn render_info( &self, device: &Device, area: Rect, buf: &mut Buffer ) {
    let block = Block::bordered().title( " Info " );

    let mut rows = Vec::with_capacity( 50 );
    rows.extend([
      Row::new([ Cell::new( "Intrinsics" ).style( Style::new().bold() ) ]),
      Row::new([ " IP address:".to_owned(), device.info().ip().to_string() ]),
      Row::new([ " MAC address:".to_owned(), device.info().mac_address().to_string() ]),
      Row::new([ " Hardware Version:".to_owned(), device.info().version().hardware.to_string() ]),
      Row::new([ " Software Version:".to_owned(), device.info().version().software.to_string() ]),
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

    Table::new( rows, [ Constraint::Length( 25 ), Constraint::Fill( 1 ) ])
      .block( block )
      .render( area, buf );
  }

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
}