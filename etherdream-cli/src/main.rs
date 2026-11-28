//! CLI tool to discover, connect and test Etherdream DAC's.
use std::io;

use etherdream::discovery;
use tokio::sync::mpsc;
use tokio::task;
use tokio_util::sync::CancellationToken;

mod app;
mod scene;
mod scenes;
mod sequencer;
mod tracks;

fn main() -> Result<(),String> {
  let rt = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .thread_name( "background" )
    .build()
    .map_err( |e| format!( "Failed to build Tokio run-time: {}", e ) )?;

  let cancellation_token = CancellationToken::new();
  let ( discovery_tx, discovery_rx ) = mpsc::channel::<discovery::Broadcast>( 16 );
  let state = app::State::new( discovery_rx );
  let ( fg, bg ) = app::build_scenes( state ).map_err( |_| "Failed to build scenes." )?;

  let thread =
    std::thread::spawn({
      let cancellation_token = cancellation_token.child_token();

      move ||{
        let _ = rt.block_on( async move {
          let mut tasks = task::JoinSet::<io::Result<()>>::new();

          // Start an Etherdream discovery server
          tasks.spawn({
            let cancellation_token = cancellation_token.child_token();

            async move {
              tokio::select!{
                _ = cancellation_token.cancelled() => Ok( () ),
                _ = async move {
                  let service = discovery::Discovery::new()
                    .notify( discovery_tx )
                    .listen().await?;

                  // TODO
                  service.join().await;

                  Ok::<(), io::Error>( () )
                } => Ok( () )
              }
            }
          });

          // Start the scene background task
          tasks.spawn({
            let cancellation_token = cancellation_token.child_token();

            async move {
              tokio::select!{
                _ = cancellation_token.cancelled() => Ok( () ),
                _ = bg.run() => Ok( () )
              }
            }
          });

          let _ = tasks.join_all().await;
        });
      }
    });

  // [Blocks] Run the foreground ui. Blocks this thread until exited by user.
  fg.run();

  // Shutdown the background processes
  cancellation_token.cancel();
  let _ = thread.join();

  Ok( () )
}

