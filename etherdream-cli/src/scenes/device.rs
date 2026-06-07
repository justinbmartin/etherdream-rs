use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::{ Constraint, Layout, Rect };
use ratatui::style::{ Color, Style };
use ratatui::text::{ Line, Span };
use ratatui::widgets::{ Block, Cell, Paragraph, Row, Widget, Table, Tabs };
use ratatui_textarea::TextArea;

use crate::scene::{ Scene, SceneContext, SceneEvent, SceneManagerBuilder, SceneManager };

#[derive( Clone, Copy, Eq, Hash, PartialEq )]
#[repr( usize )]
enum DeviceSceneKey{
  Info = 0,
  Generate = 1
}

pub struct DeviceScene {
  scenes: SceneManager<DeviceSceneKey>
}

impl Default for DeviceScene {
  fn default() -> Self {
    let scenes = SceneManagerBuilder::<DeviceSceneKey>::new( DeviceSceneKey::Info, Box::new( DeviceInfoScene::default() ) )
      .add_scene( DeviceSceneKey::Generate, Box::new( DeviceTestScene::default() ) )
      .build();

    Self{ scenes }
  }
}

impl Scene for DeviceScene {
  fn on_key_down( &mut self, ctx: &SceneContext, key: KeyCode ) -> SceneEvent {
    match self.scenes.current_scene().on_key_down( ctx, key ) {
      SceneEvent::NotHandled => {
        match key {
          KeyCode::Left => {
            if self.scenes.current_scene_key() == DeviceSceneKey::Generate {
              self.scenes.set_scene( DeviceSceneKey::Info );
            }

            SceneEvent::Handled
          },
          KeyCode::Right => {
            if self.scenes.current_scene_key() == DeviceSceneKey::Info {
              self.scenes.set_scene( DeviceSceneKey::Generate );
            }

            SceneEvent::Handled
          },
          KeyCode::Esc | KeyCode::Char( 'q' ) => {
            SceneEvent::Exit
          }
          _ => {
            SceneEvent::NotHandled
          }
        }
      },
      event => event
    }
  }

  fn render( &mut self, ctx: &SceneContext, area: Rect, buf: &mut Buffer ) {
    if let Some( device ) = ctx.selected_device() {
      let layout = Layout::vertical([ Constraint::Length( 3 ), Constraint::Fill( 1 ) ]);
      let [ menu, content ] = area.layout( &layout );

      // Render the tabbed menu
      let menu_block = Block::bordered()
        .title( Line::raw( format!( " Device: {} ", device.info().ip() ) ).centered() );

      Tabs::new( vec![ "Info", "Generate" ])
        .block( menu_block )
        .style( Color::White )
        .highlight_style( Style::default().magenta().on_black().bold() )
        .select( self.scenes.current_scene_key() as usize )
        .divider( "|" )
        .padding( " ", " " )
        .render( menu, buf );

      // Render the scene
      self.scenes.current_scene().render( ctx, content, buf );
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Device: Info Scene

#[derive( Default )]
pub struct DeviceInfoScene;

impl Scene for DeviceInfoScene {
  fn render( &mut self, ctx: &SceneContext, area: Rect, buf: &mut Buffer ) {
    if let Some( device ) = ctx.selected_device() {
      let block = Block::bordered();
      let centered_area = block.inner( area ).centered_horizontally( Constraint::Length( 50 ) );
      block.render( area, buf );

      let content_block = Block::default();

      let mut rows = Vec::with_capacity( 50 );
      rows.extend([
        Row::new([ Cell::new( "Intrinsics" ).style( Style::new().bold() ) ]),
        Row::new([ " IP address:".to_owned(), device.info().ip().to_string() ]),
        Row::new([ " MAC address:".to_owned(), device.info().mac_address().to_string() ]),
        Row::new([ " Version:".to_owned(), format!( "Hardware: {}; Software: {};", device.info().version().hardware, device.info().version().software ) ]),
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
        .block( content_block )
        .render( centered_area, buf );
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Device Test Scene

pub struct DeviceTestScene<'a> {
  selected: usize,
  port_input: TextArea<'a>
}

impl<'a> Default for DeviceTestScene<'a> {
  fn default() -> Self {
    let mut port_input = TextArea::default();
    port_input.set_cursor_line_style( Style::default() );
    port_input.set_placeholder_text( etherdream::protocol::CLIENT_PORT.to_string() );

    Self{
      port_input,
      selected: 0
    }
  }
}

impl<'a> Scene for DeviceTestScene<'a> {
  fn on_scene_enter( &mut self ) {
    self.selected = 0;
  }

  fn on_key_down( &mut self, ctx: &SceneContext, key: KeyCode ) -> SceneEvent {
    match key {
      KeyCode::Up => {
        self.selected = self.selected.saturating_sub( 1 );
        SceneEvent::Handled
      },
      KeyCode::Down => {
        self.selected = self.selected.saturating_add( 1 ).min( 2 );
        SceneEvent::Handled
      },
      KeyCode::Enter => {
        if self.selected == 2 {
          SceneEvent::Connect( ctx.selected_device().unwrap().id() )
        } else {
          SceneEvent::NotHandled
        }
      },
      KeyCode::Esc | KeyCode::Char( 'q' ) => {
        SceneEvent::Exit
      },
      _ => {
        SceneEvent::NotHandled
      }
    }
  }

  fn render( &mut self, ctx: &SceneContext, area: Rect, buf: &mut Buffer ) {
    if let Some( _device ) = ctx.selected_device() {
      let block = Block::bordered();
      Widget::render( &block, area, buf );
      let inner_area = block.inner( area );

      let centered_area = inner_area.centered_horizontally( Constraint::Length( 50 ) );

      let [ port_area, connect_area, _ ] = centered_area.layout( &Layout::vertical([
        Constraint::Length( 3 ),
        Constraint::Length( 3 ),
        Constraint::Fill( 1 )
      ]));

      //
      let button_highlight_style = Style::default().fg( Color::Green );

      // Port override
      let style = if self.selected == 1 { button_highlight_style } else { Style::default() };
      let port_block = Block::bordered().title( " Port " ).border_style( style );
      self.port_input.set_block( port_block );
      Widget::render( &self.port_input, port_area, buf );

      // Connect
      let style = if self.selected == 2 { button_highlight_style } else { Style::default() };
      let connect_block = Block::bordered().border_style( style );
      let connect_btn = Paragraph::new( Span::styled( "<C>onnect", Style::default().bold() ) ).centered().block( connect_block );
      Widget::render( connect_btn, connect_area, buf );
    }
  }
}