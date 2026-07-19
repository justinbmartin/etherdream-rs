use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::style::{ Color, palette::tailwind::SLATE, Style };
use ratatui::text::Span;
use ratatui::widgets::{ Block, Cell, Padding, Paragraph, Row, Widget, Table, TableState };
use ratatui_textarea::TextArea;

use crate::device::Device;
use crate::scene;
use super::SceneContext;

const HIGHLIGHT_STYLE: Style = Style::new().bg( SLATE.c800 );
const INPUT_CONNECT_BUTTON: usize = 1;
const INPUT_PORT: usize = 0;
const TABLE_KEY_WIDTH: u16 = 25;

//#[derive( Clone, Copy, Eq, Hash, PartialEq )]
//enum SceneKey{ ConnectForm, Generator, GeneratorList }

pub struct DeviceScene {
  scenes: scene::Controller<SceneContext>,
  state: etherdream::State
}

impl Default for DeviceScene {
  fn default() -> Self {
    let mut builder = scene::Builder::new();
    builder.add_scene( "generator", Box::new( GeneratorListScene::default() ) );

    Self{
      scenes: builder.build(),
      state: etherdream::State::default()
    }
  }
}

impl scene::Scene<SceneContext> for DeviceScene {
  fn on_key_down( &mut self, ctx: &SceneContext, key: KeyEvent ) -> scene::Event {
    let handled = self.scenes.key_down( ctx, key );

    // Change scene if this is a connect event
    //if let scene::Event::Connect( _ ) = handled {
    //  self.scenes.set_scene( SceneKey::GeneratorList );
    //};

    handled
  }

  fn on_render( &mut self, ctx: &SceneContext, area: Rect, buf: &mut Buffer ) {
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
      let test_block = Block::bordered();
      let test_inner_area = test_block.inner( test_area );
      test_block.render( test_area, buf );

      //
      self.scenes.render( ctx, test_inner_area, buf );

      // Render the info pane
      self.render_info( &device, info_area, buf );
    }
  }
}

impl DeviceScene {
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

    Table::new( intrinsic_rows, [ Constraint::Length( TABLE_KEY_WIDTH ), Constraint::Fill( 1 ) ])
      .block( intrinsics_block )
      .render( intrinsics_area, buf );

    // Render state
    let state_block = Block::bordered().title( " State " ).padding( Padding::horizontal( 1 ) );
    let mut rows = Vec::with_capacity( 50 );
    rows.push( Row::new([ "Connected:", if device.is_connected() { "Yes" } else { "No" } ]) );

    if let Some( generator ) = device.generator() {
      generator.clone_state_into( &mut self.state );

      rows.extend([
        Row::new([ "Generator:", "Demo" ]),
        Row::new([ "Points buffered:".to_owned(), self.state.points_buffered().to_string() ]),
        Row::new([ "Points per second:".to_owned(), self.state.points_per_second().to_string() ])
      ]);
    } else {
      rows.push( Row::new([ "Generator:", "None" ]) );
    }

    Table::new( rows, [ Constraint::Length( TABLE_KEY_WIDTH ), Constraint::Fill( 1 ) ])
      .block( state_block )
      .render( state_area, buf );
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Connect Scene

// Scene that renders a form to connect to an Etherdream device.
pub struct ConnectFormScene<'a> {
  input_selected: usize,
  port_input: TextArea<'a>
}

impl<'a> Default for ConnectFormScene<'a> {
  fn default() -> Self {
    let mut port_input = TextArea::default();
    port_input.set_cursor_line_style( Style::default() );
    port_input.set_placeholder_text( etherdream::protocol::CLIENT_PORT.to_string() );

    Self{
      input_selected: INPUT_CONNECT_BUTTON,
      port_input
    }
  }
}

impl<'a> scene::Scene<SceneContext> for ConnectFormScene<'a> {
  fn on_enter( &mut self ) {
    self.input_selected = INPUT_CONNECT_BUTTON;
  }

  fn on_key_down( &mut self, ctx: &mut SceneContext, key: KeyEvent ) -> scene::Event {
    match key.code {
      KeyCode::Up => {
        self.input_selected = INPUT_PORT;
        return scene::Event::Handled;
      },
      KeyCode::Down => {
        self.input_selected = INPUT_CONNECT_BUTTON;
        return scene::Event::Handled;
      },
      KeyCode::Enter => {
        if self.input_selected == INPUT_CONNECT_BUTTON && let Some( mut device ) = ctx.selected_device_mut() {
          let handle = tokio::runtime::Handle::current();

          match handle.block_on( async { device.connect().await }) {
            Ok( () ) => scene::Event::Handled,
            Err( _err ) => scene::Event::Handled
          }
        } else {
          scene::Event::NotHandled
        }
      },
      //KeyCode::Esc | KeyCode::Char( 'q' ) => return sceneEvent::Exit,
      _ => {
        if self.input_selected == 0 && self.port_input.input( key ) {
          let _is_valid = validate_port( &mut self.port_input );
          return scene::Event::Handled;
        }
      }
    };

    scene::Event::NotHandled
  }

  fn on_render( &mut self, _ctx: &SceneContext, area: Rect, buf: &mut Buffer ) {
    let centered_area = area.centered_horizontally( Constraint::Length( 50 ) );

    let [ port_area, connect_area, _ ] = centered_area.layout( &Layout::vertical([
      Constraint::Length( 3 ),
      Constraint::Length( 3 ),
      Constraint::Fill( 1 )
    ]));

    //
    let button_highlight_style = Style::default().fg( Color::Green );

    // Port input
    let style = if self.input_selected == 0 { button_highlight_style } else { Style::default() };
    let port_block = Block::bordered().title( " Port " ).border_style( style );
    self.port_input.set_block( port_block );

    Widget::render( &self.port_input, port_area, buf );

    // Connect button
    let style = if self.input_selected == INPUT_CONNECT_BUTTON { button_highlight_style } else { Style::default() };
    let connect_block = Block::bordered().border_style( style );

    Paragraph::new( Span::styled( "<C>onnect", Style::default().bold() ) )
      .centered()
      .block( connect_block )
      .render( connect_area, buf );
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Generate List Scene

struct GeneratorListScene {
  generators: Vec<String>,
  state: TableState
}

impl Default for GeneratorListScene {
  fn default() -> Self {
    Self{
      generators: vec![ "Demo".to_owned() ],
      state: TableState::new().with_selected( Some( 0 ) )
    }
  }
}

impl scene::Scene<SceneContext> for GeneratorListScene {
  fn on_key_down( &mut self, ctx: &SceneContext, key: KeyEvent ) -> scene::Event {
    match key.code {
      KeyCode::Up => {
        self.state.select( Some( self.state.selected().unwrap().saturating_sub( 1 ) ) );
        return scene::Event::Handled;
      },
      KeyCode::Down => {
        self.state.select( Some( self.state.selected().unwrap().saturating_add( 1 ) % self.generators.len() ) );
        return scene::Event::Handled;
      },
      KeyCode::Enter => {
        if let Some( _device ) = ctx.selected_device() {
          //return SceneEvent::Play( device.id() );
          return scene::Event::Change( "generator" )
        }
      },
      _ => {}
    }

    scene::Event::NotHandled
  }

  fn on_render( &mut self, _ctx: &SceneContext, area: Rect, buf: &mut Buffer ) {
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

impl scene::Scene<SceneContext> for GeneratorScene {
  fn on_key_down( &mut self, ctx: &SceneContext, key: KeyEvent ) -> scene::Event {
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

    scene::Event::NotHandled
  }

  fn on_render( &mut self, _ctx: &SceneContext, _area: Rect, _buf: &mut Buffer ) {

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