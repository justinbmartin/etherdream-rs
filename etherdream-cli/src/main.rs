//! CLI tool to discover, connect and test Etherdream DAC's.
use tokio::runtime;
use tokio_util::sync::CancellationToken;

mod ui;
mod device;
mod executors;
mod scene;
mod scenes;

fn main() -> Result<(),String> {
  let rt = runtime::Builder::new_multi_thread()
    .thread_name( "async" )
    .enable_all()
    .build()
    .unwrap();

  let cancellation_token = CancellationToken::new();
  let device_id = std::sync::Arc::new( std::sync::Mutex::new( None::<usize> ) );
  let device_map = std::sync::Arc::new( tokio::sync::Mutex::new( device::DeviceMap::default() ) );
  let ( action_tx, action_rx ) = tokio::sync::mpsc::channel( 1024 );
  let ( discovery_tx, mut discovery_rx ) = tokio::sync::mpsc::channel( 16 );
  let ( event_tx, event_rx ) = tokio::sync::mpsc::channel( 1024 );
  let main_scene = ui::MainScene::new(device_id.clone(), device_map.clone() );

  let device_map2 = device_map.clone();

  let rt_thread = std::thread::spawn({
    let cancellation_token = cancellation_token.child_token();

    move ||{
      let _ = rt.block_on( async move {

        // Start the Etherdream device discovery service
        let discovery = match etherdream::discover( discovery_tx ).await {
          Ok( server ) => server,
          Err( err ) => {
            return Err( format!( "Failed to start Etherdream device discovery service: {:?}", err ) );
          }
        };

        let h = tokio::spawn({
          let cancellation_token = cancellation_token.child_token();

          async move {
            cancellation_token.run_until_cancelled( async move {
              scene::EventController::run( event_tx, action_rx, main_scene ).await
            }).await;
          }
        });

        let i = tokio::spawn({
          let cancellation_token = cancellation_token.child_token();

          async move {
            cancellation_token.run_until_cancelled( async move {
              // Persist any discovered devices from the Etherdream discovery service
              while let Some( device_info ) = discovery_rx.recv().await {
                device_map2.lock().await.insert( *device_info.info() );
              }
            }).await;
          }
        });

        // Shutdown the discovery service and terminate
        let _ = h.await;
        let _ = i.await;
        discovery.shutdown().await;

        Ok( () )
      });
    }
  });

  // [Blocks] Create and run the app
  let terminal = ratatui::init();
  ui::UI::new( action_tx, device_id.clone(), device_map.clone() ).run( terminal, event_rx );
  ratatui::restore();

  cancellation_token.cancel();
  let _ = rt_thread.join();

  Ok( () )
}