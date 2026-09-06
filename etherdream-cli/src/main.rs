//! CLI tool to discover, connect and test Etherdream DAC's.
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use etherdream::discovery;
use tokio::sync::{ mpsc, RwLock };
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

  //
  let ( discovery_tx, mut discovery_rx ) = mpsc::channel::<SocketAddr>( 16 );
  let device_registry = discovery::Registry::default();

  let cancellation_token = CancellationToken::new();
  let state = Arc::new( RwLock::new( ui::State::new( device_registry.clone() ) ) );
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
                let discovery = discovery::Builder::new( device_registry )
                  .notify( discovery_tx );

                if let Ok( _ ) = discovery.listen().await {
                  while let Some( address ) = discovery_rx.recv().await {
                    state.write().await.add_device_to_map( address ).await;
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

