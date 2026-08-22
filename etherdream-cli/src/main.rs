//! CLI tool to discover, connect and test Etherdream DAC's.
use tokio::{ runtime, task };
use tokio_util::sync::CancellationToken;

mod ui;
mod executors;
mod scene;
mod scenes;
mod state;

fn main() -> std::io::Result<()> {
  let cancellation_token = CancellationToken::new();
  let state = state::State::default();

  // Create the event server
  let ( event_client, event_server ) = scene::make_event_server( state.clone() );

  let rt_thread =
    std::thread::spawn({
      let cancellation_token = cancellation_token.child_token();
      let device_map = state.device_map.clone();

      move ||{
        let rt = runtime::Builder::new_multi_thread()
          .thread_name( "scene driver" )
          .enable_all()
          .build()
          .unwrap();

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
              cancellation_token.run_until_cancelled( event_server.run() ).await;
            }
          });

          // Shutdown the discovery service and terminate
          let _ = tasks.join_all().await;
          Ok( () )
        });
      }
    });

  // [Blocks] Create and run the app
  ui::run( event_client, state );


  cancellation_token.cancel();
  let _ = rt_thread.join();

  Ok( () )
}