//! CLI tool to discover, connect and test Etherdream DAC's.
use std::io;
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

mod device;
mod executors;
mod scene;
mod ui;

fn main() -> Result<(),String> {
  let rt = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .thread_name( "background" )
    .build()
    .map_err( |e| format!( "Failed to build Tokio run-time: {}", e ) )?;

  let cancellation_token = CancellationToken::new();
  let state = Arc::new( RwLock::new( ui::State::default() ) );
  let ( fg, bg ) = ui::build_scenes( state.clone() ).map_err( |e| format!( "Failed to build scenes: {}", e ) )?;

  let rt_thread =
    std::thread::spawn({
      let cancellation_token = cancellation_token.child_token();

      move ||{
        let _: io::Result<()> = rt.block_on( async move {
          let mut tasks = JoinSet::<()>::new();

          // Start an Etherdream discovery server
          tasks.spawn({
            let cancellation_token = cancellation_token.child_token();
            let state = state.clone();

            async move {
              cancellation_token.run_until_cancelled( async move {
                if let Ok( mut discovery ) = etherdream::discover().await {
                  while let Some( ( device_info, _ ) ) = discovery.recv().await {
                    state.write().await.add_device_to_map( device_info );
                  }
                }
              }).await;
            }
          });

          // Start the scene background task
          tasks.spawn({
            let cancellation_token = cancellation_token.child_token();
            async move { cancellation_token.run_until_cancelled( bg.run() ).await; }
          });

          let _ = tasks.join_all().await;
          Ok( () )
        });
      }
    });

  // [Blocks] Run the foreground ui. Blocks this thread until exited by user.
  fg.run();

  // Shutdown the background processes
  cancellation_token.cancel();
  let _ = rt_thread.join();

  Ok( () )
}

