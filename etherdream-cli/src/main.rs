//! CLI tool to discover, connect and test Etherdream DAC's.
mod app;
mod device;
mod executors;
mod scene;
mod scenes;

use tokio::runtime::Runtime;
use crate::scene::Actionable;

fn main() -> Result<(),String> {
  let rt = Runtime::new().unwrap(); // TODO

  let cancellation_token = tokio_util::sync::CancellationToken::new();
  let device_id = std::sync::Arc::new( std::sync::Mutex::new( None::<usize> ) );
  let device_map = std::sync::Arc::new( std::sync::Mutex::new( device::DeviceMap::default() ) );
  let ( action_tx, mut action_rx ) = tokio::sync::mpsc::channel( 16 );
  let ( discovery_tx, discovery_rx ) = tokio::sync::mpsc::channel( 16 );
  let ( event_tx, event_rx ) = tokio::sync::mpsc::channel( 16 );
  let mut main_scene = app::MainScene::new( device_id.clone(), device_map.clone() );

  rt.spawn({
    let cancellation_token = cancellation_token.child_token();
    let cancellation_token2 = cancellation_token.child_token();
    let device_id = device_id.clone();
    let device_map = device_map.clone();
    let event_tx2 = event_tx.clone();

    async move {
      let events_controller = scene::EventController::start( event_tx, cancellation_token ).await;

      // Start the Etherdream device discovery service
      let discovery = match etherdream::discover( discovery_tx ).await {
        Ok( server ) => server,
        Err( err ) => {
          return Err( format!( "Failed to start Etherdream device discovery service: {:?}", err ) );
        }
      };

      let handle = tokio::spawn( async move {
        tokio::select!{
          _ = cancellation_token2.cancelled() => {}
          _ = async move {
              while let Some( action ) = action_rx.recv().await {
                let event = main_scene.invoke( action ).await;
                let _ = event_tx2.send( scene::Event::Scene( event ) ).await;
              }
            } => {}
        }
      });

      // Shutdown the discovery service and terminate
      let _ = handle.await;
      events_controller.stop().await;
      discovery.shutdown().await;

      Ok( () )
    }
  });

  // [Blocks] Create and run the app
  let terminal = ratatui::init();
  app::App::new( action_tx ).run( terminal, discovery_rx, event_rx );
  ratatui::restore();

  Ok( () )
}