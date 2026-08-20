//! CLI tool to discover, connect and test Etherdream DAC's.
use tokio::{ runtime, sync::mpsc, task };
use tokio_util::sync::CancellationToken;

mod ui;
mod device;
mod executors;
mod scene;
mod scenes;

fn main() -> std::io::Result<()> {
  let rt = runtime::Builder::new_multi_thread()
    .thread_name( "scene driver" )
    .enable_all()
    .build()?;

  let cancellation_token = CancellationToken::new();
  let device_id = std::sync::Arc::new( std::sync::Mutex::new( None::<usize> ) );
  let device_map = std::sync::Arc::new( tokio::sync::Mutex::new( device::DeviceMap::default() ) );
  let ( action_tx, action_rx ) = mpsc::channel( 16 );
  let ( event_tx, event_rx ) = mpsc::channel( 1024 );
  let main_scene = ui::MainScene::new(device_id.clone(), device_map.clone() );

  let rt_thread = std::thread::spawn({
    let cancellation_token = cancellation_token.child_token();
    let device_map = device_map.clone();

    move ||{
      let _: Result<(),std::io::Error> = rt.block_on( async move {
        let mut tasks = task::JoinSet::<()>::new();

        tasks.spawn({
          let cancellation_token = cancellation_token.child_token();
          let device_map = device_map.clone();

          async move {
            cancellation_token.run_until_cancelled( async move {
              if let Ok( mut device_rx ) = etherdream::discover().await {
                while let Some( device_info ) = device_rx.recv().await {
                  device_map.lock().await.insert( *device_info.info() );
                }
              }
            }).await;
          }
        });

        tasks.spawn({
          let cancellation_token = cancellation_token.child_token();

          async move {
            cancellation_token.run_until_cancelled( async move {
              scene::EventController::new( event_tx, action_rx, main_scene ).run().await
            }).await;
          }
        });

        // Shutdown the discovery service and terminate
        let _ = tasks.join_all().await;
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