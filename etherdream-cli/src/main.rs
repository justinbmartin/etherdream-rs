//! CLI tool to discover, connect and test Etherdream DAC's.
use std::io;

use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

mod ui;
mod executors;
mod scene;
mod scenes;
mod state;

fn main() -> io::Result<()> {
  let rt = tokio::runtime::Builder::new_multi_thread()
    .thread_name( "scene-event-server" )
    .enable_all()
    .build()?;
  
  let cancellation_token = CancellationToken::new();
  let state = state::State::default();

  // Create the event server
  let ( event_client, event_server ) = scene::make_event_server( state.clone() );

  let rt_thread =
    std::thread::spawn({
      let cancellation_token = cancellation_token.child_token();
      let device_map = state.device_map.clone();

      move ||{
        let _: io::Result<()> = rt.block_on( async move {
          let mut tasks = JoinSet::<()>::new();

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
              cancellation_token.run_until_cancelled( event_server.run() ).await;
            }
          });

          // Await the tasks to complete
          let _ = tasks.join_all().await;
          Ok( () )
        });
      }
    });

  // [Blocks] Run the ui. Blocks until run is exited by user.
  ui::run( event_client, state );

  // Send cancellation to thread and await shut down
  cancellation_token.cancel();
  let _ = rt_thread.join();

  Ok( () )
}