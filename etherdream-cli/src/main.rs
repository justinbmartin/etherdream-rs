//! CLI tool to discover, connect and test Etherdream DAC's.
mod app;
mod device;
mod executors;
mod scene;
mod scenes;

use tokio::runtime::Runtime;

fn main() -> Result<(),String> {
  let cancellation_token = tokio_util::sync::CancellationToken::new();
  let device_id = std::sync::Arc::new( std::sync::Mutex::new( None::<usize> ) );
  let device_map = std::sync::Arc::new( std::sync::Mutex::new( device::DeviceMap::default() ) );
  let ( action_tx, action_rx ) = tokio::sync::mpsc::channel( 1024 );
  let ( discovery_tx, discovery_rx ) = tokio::sync::mpsc::channel( 16 );
  let ( event_tx, event_rx ) = tokio::sync::mpsc::channel( 1024 );
  let main_scene = app::MainScene::new( device_id.clone(), device_map.clone() );

  let cancellation_token8 = cancellation_token.child_token();

  let rt_thread = std::thread::spawn( move ||{
    let rt = tokio::runtime::Builder::new_multi_thread()
      .thread_name( "async" )
      .enable_all()
      .build()
      .unwrap();

    let _ = rt.block_on({
      let cancellation_token1 = cancellation_token8.child_token();
      let cancellation_token2 = cancellation_token8.child_token();

      async move {

        // Start the Etherdream device discovery service
        let discovery = match etherdream::discover( discovery_tx ).await {
          Ok( server ) => server,
          Err( err ) => {
            return Err( format!( "Failed to start Etherdream device discovery service: {:?}", err ) );
          }
        };

        let h = tokio::spawn( async move {
          tokio::select!{
            _ = cancellation_token1.cancelled() => { }
            _ = scene::EventController::run(
                event_tx,
                cancellation_token2,
                action_rx,
                main_scene
              ) => { }
          }
        });

        // Shutdown the discovery service and terminate
        let _ = h.await;
        discovery.shutdown().await;

        Ok( () )
      }
    });
  });

  // [Blocks] Create and run the app
  let terminal = ratatui::init();
  app::App::new( action_tx, device_id.clone(), device_map.clone() ).run( terminal, discovery_rx, event_rx );
  cancellation_token.cancel();
  ratatui::restore();

  let _ = rt_thread.join();

  Ok( () )
}