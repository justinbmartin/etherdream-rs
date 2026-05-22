//! CLI tool to discover, connect and test Etherdream DAC's.
mod executors;

use std::cell::{ Ref, RefCell };
use std::collections::HashMap;
use std::net::SocketAddr;
use std::rc::Rc;
use std::time::Duration;

use crossterm::event::{ self, Event, KeyCode, KeyEvent, KeyEventKind };
use ratatui::prelude::*;
use ratatui::style::palette::tailwind::SLATE;
use ratatui::widgets::{ Block, Paragraph, Row, Table, TableState };

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Main

#[tokio::main]
async fn main() {
  let mut app = App::default();

  // Start the Etherdream discovery service
  let ( device_info_tx, mut device_info_rx ) = tokio::sync::mpsc::channel( 16 );

  let _discovery_server =
    match etherdream::discover( device_info_tx ).await {
      Ok( server ) => server,
      Err( err ) => {
        eprintln!( "Failed to start Etherdream discovery: {:?}", err );
        return;
      }
    };

  ratatui::run(| terminal |{
    loop {
      if app.should_exit { break; }

      // Capture any discovered devices from the Etherdream discovery service
      while let Ok( device_info ) = device_info_rx.try_recv() {
        app.add_device( device_info.info().clone() );
      }

      // Render
      let _ = terminal.draw(| frame |{ app.render( frame ); });

      // Handle any user-input
      //
      // We poll here to ensure we do not block on event `read`. This ensures
      // that we always handle any discovered devices from `device_info_rx`.
      if let Ok( true ) = event::poll( Duration::from_millis( 100 ) ) {
        if let Ok( Event::Key( key ) ) = event::read() {
          app.on_key_event( key );
        }
      }
    }
  });
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

#[derive( Eq, Hash, PartialEq )]
enum Scene { List, Info }

#[derive( PartialEq )]
enum OnKeyEventResult {
  Select( usize ),
  Back,
  Handled,
  NotHandled
}

trait IsScene {
  fn on_key_press( &mut self, key: KeyCode ) -> OnKeyEventResult;
  fn render( &mut self, area: Rect, buf: &mut Buffer );
}

struct SceneData {
  device_infos: Rc<RefCell<Vec<etherdream::DeviceInfo>>>,
  device_selected: Rc<RefCell<Option<usize>>>
}

impl SceneData {
  fn device_infos( &'_ self ) -> Ref<'_, Vec<etherdream::DeviceInfo>> {
    self.device_infos.borrow()
  }

  fn selected_device( &self ) -> Option<usize> {
    *self.device_selected.borrow()
  }
}

impl Clone for SceneData {
  fn clone( &self ) -> Self {
    SceneData{
      device_infos: self.device_infos.clone(),
      device_selected: self.device_selected.clone()
    }
  }
}

struct App {
  current_scene: Scene,
  scene_data: SceneData,
  scenes: HashMap<Scene,Box<dyn IsScene>>,
  should_exit: bool
}

impl Default for App {
  fn default() -> Self {
    // TODO: Maybe wrap in `test` configuration option?
    let device_infos = Rc::new( RefCell::new( vec![
      etherdream::DeviceInfo::new( SocketAddr::from(( [10, 0, 0, 1], 6543 )), etherdream::protocol::Intrinsics::default() ),
      etherdream::DeviceInfo::new( SocketAddr::from(( [10, 0, 0, 2], 6543 )), etherdream::protocol::Intrinsics::default() )
    ] ) );

    let device_selected = Rc::new( RefCell::new( None::<usize> ) );

    let scene_data = SceneData{ device_infos, device_selected };

    let mut scenes: HashMap<Scene,Box<dyn IsScene>> = HashMap::new();
    scenes.insert( Scene::Info, Box::new( InfoScene::new( scene_data.clone() ) ) );
    scenes.insert( Scene::List, Box::new( ListScene::new( scene_data.clone() ) ) );

    Self{
      current_scene: Scene::List,
      scene_data,
      scenes,
      should_exit: false
    }
  }
}

impl App {
  fn add_device( &mut self, device_info: etherdream::DeviceInfo ) {
    self.scene_data.device_infos.borrow_mut().push( device_info )
  }

  fn on_key_event( &mut self, key: KeyEvent ) {
    if key.kind == KeyEventKind::Press {
      let handled =
        if let Some( scene ) = self.scenes.get_mut( &self.current_scene ) {
          scene.on_key_press( key.code )
        } else {
          OnKeyEventResult::NotHandled
        };

      match handled {
        OnKeyEventResult::Select( index ) => {
          *self.scene_data.device_selected.borrow_mut() = Some( index );
          self.current_scene = Scene::Info;
        },
        OnKeyEventResult::Back => {
          *self.scene_data.device_selected.borrow_mut() = None;
          self.current_scene = Scene::List;
        },
        OnKeyEventResult::NotHandled => {
          match key.code {
            KeyCode::Char( 'q' ) | KeyCode::Esc => {
              self.should_exit = true;
            },
            _ => {}
          }
        }
        OnKeyEventResult::Handled => { /* no-op */ }
      };
    }
  }

  fn render( &mut self, frame: &mut Frame ) {
    let main_layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
    let [ content_area, footer_area ] = frame.area().layout( &main_layout );

    if let Some( scene ) = self.scenes.get_mut( &self.current_scene ) {
      scene.render( content_area, frame.buffer_mut() )
    }

    // Main > Footer
    Paragraph::new( "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." )
      .centered()
      .render( footer_area, frame.buffer_mut() );
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene: List

struct ListScene {
  data: SceneData,
  state: TableState
}

impl ListScene {
  fn new( data: SceneData ) -> Self {
    let mut state = TableState::default();
    state.select( Some( 0 ) );

    Self{ data, state }
  }
}

impl IsScene for ListScene {
  fn on_key_press( &mut self, key: KeyCode ) -> OnKeyEventResult {
    match key {
      KeyCode::Down => {
        let i =
          if let Some( i ) = self.state.selected() {
            i.saturating_add( 1 ) % self.data.device_infos().len()
          } else {
            0
          };

        self.state.select( Some( i ) );
        return OnKeyEventResult::Handled;
      },
      KeyCode::Up => {
        let i =
          if let Some( i ) = self.state.selected() {
            i.saturating_sub( 1 ) % self.data.device_infos().len()
          } else {
            0
          };

        self.state.select( Some( i ) );
        return OnKeyEventResult::Handled;
      },
      KeyCode::Enter => {
        if let Some( index ) = self.state.selected() {
          return OnKeyEventResult::Select( index );
        }
      }
      _ => {}
    }

    // Bubble-up all other key events
    OnKeyEventResult::NotHandled
  }

  fn render( &mut self, area: Rect, buf: &mut Buffer ) {
    let block = Block::bordered().title( Line::raw( " Etherdream Devices " ).centered() );

    if self.data.device_infos().is_empty() {
      Paragraph::new( "(no devices)" )
        .centered()
        .block( block )
        .render( area, buf )
    } else {
      let devices: Vec<Row> = self.data.device_infos()
        .iter()
        .map(|di|{ Row::new([
          di.address().to_string(),
          di.mac_address().to_string(),
        ]) }).collect();

      let constraints = [ Constraint::Length( 25 ), Constraint::Fill( 1 ) ];

      let table = Table::new( devices, constraints )
        .block( block )
        .header( Row::new(vec![ "Address", "MAC" ]).style( Style::new().bold() ) )
        .row_highlight_style( Style::new().bg( SLATE.c800 ).add_modifier( Modifier::BOLD ) )
        .highlight_symbol( "> " )
        .highlight_spacing( ratatui::widgets::HighlightSpacing::Always );

      // We need to disambiguate this trait method as both `Widget` and `StatefulWidget` share the
      // same method name `render`.
      StatefulWidget::render( table, area, buf, &mut self.state );
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene: Info

struct InfoScene {
  data: SceneData
}

impl InfoScene {
  fn new( data: SceneData ) -> Self {
    Self{ data }
  }
}

impl IsScene for InfoScene {
  fn on_key_press( &mut self, key: KeyCode ) -> OnKeyEventResult {
    match key {
      KeyCode::Esc | KeyCode::Char( 'q' ) => OnKeyEventResult::Back,
      _ => OnKeyEventResult::NotHandled
    }
  }

  fn render( &mut self, area: Rect, buf: &mut Buffer ) {
    if let Some( device_index ) = self.data.selected_device() &&
       let Some( device_info ) = self.data.device_infos().get( device_index ) {
      let block = Block::bordered().title( Line::raw( format!( " Device: {} ", device_info.address() ) ).centered() );

      Paragraph::new( format!( "MAC Address: {}", device_info.mac_address() ) )
        .centered()
        .block( block )
        .render( area, buf )
    }
  }
}