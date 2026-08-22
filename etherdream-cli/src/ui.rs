use crossterm::event::KeyCode;
use ratatui::layout::{ Constraint, Layout };
use ratatui::widgets::{ Paragraph, Widget };
use tokio::sync::mpsc::Sender;

use crate::scene;
use crate::scenes;
use crate::state::{ self, Action, State };

pub fn run( mut event_client: scene::EventClient<Action>, state: State ) {
  let mut controller = make_scenes( state.clone(), event_client.get_action_tx() );

  // Initialize the ratatui context
  let mut terminal = ratatui::init();

  // [Blocks] Wait for events from the `EventServer` and process as required
  while let Some( event ) = event_client.event_rx.blocking_recv() {
    match event {
      scene::Event::Key( key ) => {
        if ! controller.key_down( key ) {
          match key.code {
            KeyCode::Char( 'q' ) | KeyCode::Esc => { break; },
            _ => { }
          }
        }
      },
      scene::Event::Tick( _time ) => {
        let _ = controller.update();

        let _ = terminal.draw(| frame |{
          let main_layout = Layout::vertical([ Constraint::Fill( 1 ), Constraint::Length( 1 ) ]);
          let [ body_area, footer_area ] = frame.area().layout( &main_layout );

          // Main > Body
          controller.draw( body_area, frame.buffer_mut() );

          // Main > Footer
          Paragraph::new( "Use ↓↑ to move, <Enter> to select a device, 'q' to quit." )
            .centered()
            .render( footer_area, frame.buffer_mut() );
        });
      }
      scene::Event::Scene( event ) => {
        controller.on_event( event )
      }
    }
  }

  // Restore the terminal to its pre-existing state
  ratatui::restore();
}

fn make_scenes( state: State, action_tx: scene::ActionTx<Action> ) -> scene::Controller<State> {
  let mut builder = scene::Builder::new();

  {
    let device_map = state::ReadOnlyDeviceMap::new( state.device_map.clone() );
    builder.add_scene( scenes::list::ID, Box::new( scenes::list::ListScene::new( device_map ) ) );
  }

  {
    let scoped_device = state::ScopedDevice::new( state.device_id.clone(), state.device_map.clone() );
    builder.add_scene( scenes::device::ID, Box::new( scenes::device::DeviceScene::new( scoped_device ) ) );
  }

  {
    let scoped_device = state::ScopedDevice::new( state.device_id.clone(), state.device_map.clone() );
    builder.add_scene( scenes::connect::ID, Box::new( scenes::connect::ConnectScene::new( scoped_device ) ) );
  }

  builder.build( action_tx )
}