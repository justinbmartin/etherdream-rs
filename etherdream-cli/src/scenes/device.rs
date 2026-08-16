use crossterm::event::{ KeyCode, KeyEvent };
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::style::{ Color, palette::tailwind::SLATE, Style };
//use ratatui::text::Span;
use ratatui::widgets::{ Block, /* Cell, */ Padding, Paragraph, Row, Widget, Table, /* TableState */ };
use ratatui_textarea::TextArea;

use crate::app;
use crate::device;
use crate::scene;

const HIGHLIGHT_STYLE: Style = Style::new().bg( SLATE.c800 );
const TABLE_KEY_WIDTH: u16 = 25;

pub struct DeviceScene {
  device: device::ScopedDevice
}

impl DeviceScene {
  pub fn new( device: device::ScopedDevice ) -> Self {
    Self{ device }
  }
}

impl scene::Scene<app::Handler> for DeviceScene {
  fn on_draw( &mut self, area: Rect, buf: &mut Buffer ) {
    if let Some( device ) = self.device.get() {
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
      //let test_inner_area = test_block.inner( test_area );
      test_block.render( test_area, buf );

      //
      //self.scenes.render( test_inner_area, buf );

      // Render the info pane
      render_info( &device, info_area, buf );
    }
  }
}

async fn on_key_down( _key: KeyEvent, _device: Option<&device::Device> ) -> bool {
  //let handled = self.scenes.key_down( key );

  // Change scene if this is a connect event
  //if let scene::Event::Connect( _ ) = handled {
  //  self.scenes.set_scene( SceneKey::GeneratorList );
  //};

  //handled
  false
}

// UI to render the Etherdream device intrinsic and run-time properties
fn render_info( device: &device::DeviceGuard, area: Rect, buf: &mut Buffer ) {
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

  Table::new( rows, [ Constraint::Length( TABLE_KEY_WIDTH ), Constraint::Fill( 1 ) ])
    .block( state_block )
    .render( state_area, buf );
}


/*
mod connect {
  use std::sync::{ Arc, Mutex };

  use crossterm::event::{ KeyCode, KeyEvent };

  use crate::app;
  use crate::scene;

  pub struct State {
    pub connecting: Option<bool>,
  }

  pub async fn on_key_down( key: KeyEvent, _device_map: Arc<Mutex<crate::device::DeviceMap>> ) -> bool {
    if key.code == KeyCode::Char( 'c' ) {

      //state.connecting = Some( true );
      return true;
    }

    false
  }

  pub async fn on_update() -> scene::Event<app::Event> {
    scene::Event::NoChange
  }

  pub fn on_render( area: Rect, buf: &mut Buffer ) {
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

pub fn make_connect_scene_definition( device_map: Arc<Mutex<crate::device::DeviceMap>> ) -> scene::SceneDefinition<app::Event> {
  scene::SceneDefinition{
    on_key_down: Some(
      Box::new( move | e: KeyEvent |{
        let dm = device_map.clone(); // think...
        Box::pin( connect::on_key_down( e, dm ) )
      })
    ),
    on_update: Some( Box::new( move ||{ Box::pin( connect::on_update() ) }) ),
    on_render: Box::new( | _area, _buf |{} )
  }
}

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