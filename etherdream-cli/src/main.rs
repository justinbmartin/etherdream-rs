//! CLI tool to discover, connect and test Etherdream DAC's.
mod app;
mod device;
mod event;
mod executors;
mod scene;

#[tokio::main]
async fn main() -> Result<(),String> {
  // Start the Etherdream device discovery service
  let ( discovery_tx, discovery_rx ) = tokio::sync::mpsc::channel( 16 );

  let discovery =
    match etherdream::discover( discovery_tx ).await {
      Ok( server ) => server,
      Err( err ) => {
        return Err( format!( "Failed to start Etherdream device discovery service: {:?}", err ) );
      }
    };

  // [Blocks] Create and run the app
  let terminal = ratatui::init();
  app::App::new().run( terminal, discovery_rx ).await;
  ratatui::restore();

  // Shutdown the discovery service and terminate
  discovery.shutdown().await;
  Ok( () )
}