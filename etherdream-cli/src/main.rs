//! CLI tool to discover, connect and test Etherdream DAC's.
use std::io;
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

mod device;
mod executors;
mod read_only;
mod scene;
mod state;
mod ui;

fn main() -> io::Result<()> {
  let rt = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .thread_name( "background" )
    .build()?;

  let cancellation_token = CancellationToken::new();
  let device_map = Arc::new( RwLock::new( device::DeviceMap::new() ) );
  let state = state::State::new( device_map.clone() );

  let ( mut ui, server ) = scene::init( ui::build_scenes, state.clone() );

  let rt_thread =
    std::thread::spawn({
      let cancellation_token = cancellation_token.child_token();

      move ||{
        let _: io::Result<()> = rt.block_on( async move {
          let mut tasks = JoinSet::<()>::new();

          // Start an Etherdream discovery server
          tasks.spawn({
            let cancellation_token = cancellation_token.child_token();

            async move {
              cancellation_token.run_until_cancelled( async move {
                if let Ok( mut discovery ) = etherdream::discover().await {
                  while let Some( ( device_info, _ ) ) = discovery.recv().await {
                    device_map.write().await.insert( device_info );
                  }
                }
              }).await;
            }
          });

          // Start the scene server
          tasks.spawn({
            let cancellation_token = cancellation_token.child_token();

            async move {
              cancellation_token.run_until_cancelled( server.run() ).await;
            }
          });

          let _ = tasks.join_all().await;
          Ok( () )
        });
      }
    });

  // [Blocks] Run the ui. Blocks this thread until exited by user.
  ui.run();

  // Shutdown the background processes
  cancellation_token.cancel();
  let _ = rt_thread.join();

  Ok( () )
}