use std::cell::{ Ref, RefCell };
use std::collections::HashMap;
use std::net::SocketAddr;
use std::rc::Rc;
use std::time::Duration;

use crossterm::event::{ self, Event, KeyCode, KeyEvent, KeyEventKind };
use ratatui::prelude::*;
use ratatui::style::palette::tailwind::SLATE;
use ratatui::widgets::{ Block, Paragraph, Row, Table, TableState };
use tokio::sync::mpsc::Receiver;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

pub struct App {
  current_scene: Scene,
  device_infos: Rc<RefCell<Vec<etherdream::DeviceInfo>>>,
  device_info_rx: Receiver<etherdream::DiscoveredDeviceInfo>,
  device_selected_index: Rc<RefCell<Option<usize>>>,
  scenes: HashMap<Scene,Box<dyn IsScene>>,
  should_exit: bool
}

impl App {
  pub fn new( device_info_rx: Receiver<etherdream::DiscoveredDeviceInfo> ) -> Self {
    // TODO: Maybe wrap in `test` configuration option?
    let device_infos = Rc::new( RefCell::new( vec![
      etherdream::DeviceInfo::new( SocketAddr::from(( [10, 0, 0, 1], 6543 )), etherdream::protocol::Intrinsics::default() ),
      etherdream::DeviceInfo::new( SocketAddr::from(( [10, 0, 0, 2], 6543 )), etherdream::protocol::Intrinsics::default() )
    ] ) );

    let device_selected_index = Rc::new( RefCell::new( None::<usize> ) );

    let scene_data = SceneData{
      device_infos: device_infos.clone(),
      device_selected_index: device_selected_index.clone()
    };

    let mut scenes: HashMap<Scene,Box<dyn IsScene>> = HashMap::new();
    scenes.insert( Scene::Info, Box::new( InfoScene::new( scene_data.clone() ) ) );
    scenes.insert( Scene::List, Box::new( ListScene::new( scene_data ) ) );

    Self{
      current_scene: Scene::List,
      device_infos,
      device_info_rx,
      device_selected_index,
      scenes,
      should_exit: false
    }
  }
}

impl App {
  pub fn run( &mut self ) {
    ratatui::run(| terminal |{
      loop {
        if self.should_exit { break; }

        // Capture any discovered devices from the Etherdream discovery service
        while let Ok( device_info ) = self.device_info_rx.try_recv() {
          self.device_infos.borrow_mut().push( device_info.info().clone() )
        }

        // Render
        let _ = terminal.draw(| frame |{ self.render( frame ); });

        // Handle any user-input
        //
        // We poll here to ensure we do not block on event `read`. This ensures
        // that we always handle any discovered devices from `device_info_rx`.
        if let Ok( true ) = event::poll( Duration::from_millis( 100 ) ) {
          if let Ok( Event::Key( key ) ) = event::read() {
            self.on_key_event( key );
          }
        }
      }
    });
  }

  fn on_key_event( &mut self, key: KeyEvent ) {
    if key.kind == KeyEventKind::Press {
      let handled =
        if let Some( scene ) = self.scenes.get_mut( &self.current_scene ) {
          scene.on_key_press( key.code )
        } else {
          SceneEvent::NotHandled
        };

      match handled {
        SceneEvent::Select( index ) => {
          *self.device_selected_index.borrow_mut() = Some( index );
          self.current_scene = Scene::Info;
        },
        SceneEvent::Exit => {
          *self.device_selected_index.borrow_mut() = None;
          self.current_scene = Scene::List;
        },
        SceneEvent::NotHandled => {
          match key.code {
            KeyCode::Char( 'q' ) | KeyCode::Esc => {
              self.should_exit = true;
            },
            _ => {}
          }
        }
        SceneEvent::Handled => { /* no-op */ }
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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

// List of scenes this application contains.
#[derive( Eq, Hash, PartialEq )]
enum Scene { List, Info }

// Return values from scene key events
#[derive( PartialEq )]
enum SceneEvent {
  Exit,           // The scene should be exited
  Handled,        // The event was handled internally by the scene
  NotHandled,     // The event was not handled by the scene
  Select( usize ) // A device was selected
}

// All scenes must implement this trait
trait IsScene {
  fn on_key_press( &mut self, key: KeyCode ) -> SceneEvent;
  fn render( &mut self, area: Rect, buf: &mut Buffer );
}

// Shared read-only scene data
struct SceneData {
  device_infos: Rc<RefCell<Vec<etherdream::DeviceInfo>>>,
  device_selected_index: Rc<RefCell<Option<usize>>>
}

impl SceneData {
  // Returns a read-only reference to the list of discovered device infos
  fn device_infos( &'_ self ) -> Ref<'_, Vec<etherdream::DeviceInfo>> {
    self.device_infos.borrow()
  }

  // Returns the selected device index, if one is set
  fn device_selected_index( &self ) -> Option<usize> {
    *self.device_selected_index.borrow()
  }
}

impl Clone for SceneData {
  fn clone( &self ) -> Self {
    SceneData{
      device_infos: self.device_infos.clone(),
      device_selected_index: self.device_selected_index.clone()
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Scene: List

struct ListScene {
  data: SceneData,
  state: TableState
}

impl ListScene {
  fn new( data: SceneData ) -> Self {
    Self{
      data,
      state: TableState::new().with_selected( Some( 0 ) )
    }
  }
}

impl IsScene for ListScene {
  fn on_key_press( &mut self, key: KeyCode ) -> SceneEvent {
    match key {
      KeyCode::Down => {
        let i = self.state.selected().unwrap_or( 0 ).saturating_add( 1 ) % self.data.device_infos().len();
        self.state.select( Some( i ) );
        SceneEvent::Handled
      },
      KeyCode::Up => {
        let i = self.state.selected().unwrap_or( 0 ).saturating_sub( 1 ) % self.data.device_infos().len();
        self.state.select( Some( i ) );
        SceneEvent::Handled
      },
      KeyCode::Enter => {
        let i = self.state.selected().unwrap_or( 0 );
        SceneEvent::Select( i )
      }
      _ => {
        SceneEvent::NotHandled
      }
    }
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
  fn on_key_press( &mut self, key: KeyCode ) -> SceneEvent {
    match key {
      KeyCode::Esc | KeyCode::Char( 'q' ) => SceneEvent::Exit,
      _ => SceneEvent::NotHandled
    }
  }

  fn render( &mut self, area: Rect, buf: &mut Buffer ) {
    if let Some( index ) = self.data.device_selected_index() &&
       let Some( device_info ) = self.data.device_infos().get( index ) {
      let block = Block::bordered().title( Line::raw( format!( " Device: {} ", device_info.address() ) ).centered() );

      Paragraph::new( format!( "MAC Address: {}", device_info.mac_address() ) )
        .centered()
        .block( block )
        .render( area, buf )
    }
  }
}