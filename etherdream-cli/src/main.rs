//! CLI tool to discover, connect and test Etherdream DAC's.
mod app;
mod device;
mod event;
mod executors;
mod scene;

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
  // Start the Etherdream device discovery service
  let ( discovery_tx, discovery_rx ) = tokio::sync::mpsc::channel( 16 );

  let discovery =
    match etherdream::discover( discovery_tx ).await {
      Ok( server ) => server,
      Err( err ) => {
        eprintln!( "Failed to start Etherdream device discovery service: {:?}", err );
        return ExitCode::FAILURE;
      }
    };

  // [Blocks] Create and run the app
  let terminal = ratatui::init();
  app::App::new().run( terminal, discovery_rx ).await;

  // Shutdown the discovery service and terminate
  discovery.shutdown().await;
  ExitCode::SUCCESS
}