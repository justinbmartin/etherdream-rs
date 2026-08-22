use crossterm::event::KeyCode;
use ratatui::DefaultTerminal;
use ratatui::layout::{ Constraint, Layout };
use ratatui::widgets::{ Paragraph, Widget };
use tokio::sync::mpsc::Receiver;

use crate::scene;
use crate::scenes;
use crate::state;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - UI

pub struct UI {
  builder: scene::Builder<state::State>
}

impl UI {
  pub fn new( state: state::State ) -> Self {
    
    // Scenes
    let mut builder = scene::Builder::new();

    {
      let device_map = state::ReadOnlyDeviceMap::new( state.device_map.clone() );
      builder.add_scene( scenes::list::ID, Box::new( scenes::list::ListScene::new( device_map ) ) );
    }

    {
      let scoped_device = state::ScopedDevice::new(state.device_id.clone(), state.device_map.clone() );
      builder.add_scene( scenes::device::ID, Box::new( scenes::device::DeviceScene::new( scoped_device ) ) );
    }

    {
      let scoped_device = state::ScopedDevice::new(state.device_id.clone(), state.device_map.clone() );
      builder.add_scene( scenes::connect::ID, Box::new( scenes::connect::ConnectScene::new( scoped_device ) ) );
    }

    Self{ builder }
  }

  pub fn run( self, action_tx: tokio::sync::mpsc::Sender<state::Action>, mut terminal: DefaultTerminal, mut event_rx: Receiver<scene::Event> ) {
    let mut scenes = self.builder.build( action_tx );

    while let Some( event ) = event_rx.blocking_recv() {
      match event {
        scene::Event::Key( key ) => {
          if ! scenes.key_down( key ) {
            match key.code {
              KeyCode::Char( 'q' ) | KeyCode::Esc => { return; },
              _ => { }
            }
          }
        },
        scene::Event::Tick( _time ) => {
          let _ = scenes.update();

          let _ = terminal.draw(| frame |{
            let main_layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
            let [ body_area, footer_area ] = frame.area().layout( &main_layout );

            // Main > Body
            scenes.draw( body_area, frame.buffer_mut() );

            // Main > Footer
            Paragraph::new( "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." )
              .centered()
              .render( footer_area, frame.buffer_mut() );
          });
        }
        scene::Event::Scene( event ) => {
          scenes.on_event( event )
        }
      }
    }
  }
}