//! CLI tool to discover, connect and test Etherdream DAC's.
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use etherdream::discovery;
use tokio::sync::{ mpsc, RwLock };
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

mod device;
mod scene;
mod tracks;
mod ui;

fn main() -> Result<(),String> {
  let rt = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .thread_name( "background" )
    .build()
    .map_err( |e| format!( "Failed to build Tokio run-time: {}", e ) )?;

  let cancellation_token = CancellationToken::new();
  let ( discovery_tx, mut discovery_rx ) = mpsc::channel::<SocketAddr>( 16 );
  let state = Arc::new( RwLock::new( ui::State::new() ) );
  let ( fg, bg ) = ui::build_scenes( state.clone() );

  let thread =
    std::thread::spawn({
      let cancellation_token = cancellation_token.child_token();

      move ||{
        let _: io::Result<()> = rt.block_on( async move {
          let mut tasks = JoinSet::<Result<(),io::Error>>::new();

          // Start an Etherdream discovery server
          tasks.spawn({
            let cancellation_token = cancellation_token.child_token();

            async move {
              tokio::select!{
                _ = cancellation_token.cancelled() => Ok( () ),
                result = async move {
                  let registry = discovery::Registry::default();

                  let _ = discovery::Discovery::with_registry( registry.clone() )
                    .notify( discovery_tx )
                    .listen().await?;

                  while let Some( address ) = discovery_rx.recv().await {
                    if let Some( broadcast ) = registry.read().await.get( &address ) {
                      state.write().await.register_device( *broadcast.device_info() ).await;
                    }
                  }

                  Ok( () )
                } => result
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

          tasks.join_all().await;
          Ok( () )
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

