//! CLI tool to discover, connect and test Etherdream DAC's.
use std::io;

use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

mod device;
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
  let device_map = device::DeviceMap::new();
  let state = state::State::new( device_map.clone() );

  let ( mut ui, server ) = scene::init( scenes::build, state.clone() );

  let rt_thread =
    std::thread::spawn({
      let cancellation_token = cancellation_token.child_token();

      move ||{
        let _: io::Result<()> = rt.block_on( async move {
          let mut tasks = JoinSet::<()>::new();

          tasks.spawn({
            let cancellation_token = cancellation_token.child_token();

            async move {
              cancellation_token.run_until_cancelled( async move {
                if let Ok( mut device_rx ) = etherdream::discover().await {
                  while let Some( device_info ) = device_rx.recv().await {
                    device_map.write().await.insert( *device_info.info() );
                  }
                }
              }).await;
            }
          });

          tasks.spawn({
            let cancellation_token = cancellation_token.child_token();

            async move {
              cancellation_token.run_until_cancelled( server.run() ).await;
            }
          });

          // Await the tasks to complete
          let _ = tasks.join_all().await;
          Ok( () )
        });
      }
    });

  // [Blocks] Run the ui. Blocks until exited by user.
  ui.run();

  // Send cancellation to thread and await shut down
  cancellation_token.cancel();
  let _ = rt_thread.join();

  Ok( () )
}