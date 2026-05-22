//! CLI tool to discover, connect and test Etherdream DAC's.
mod app;
mod executors;

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
  // Start the Etherdream device discovery service
  let ( discovery_tx, discovery_rx ) = tokio::sync::mpsc::channel( 16 );

  let _ =
    match etherdream::discover( discovery_tx ).await {
      Ok( server ) => server,
      Err( err ) => {
        eprintln!( "Failed to start Etherdream device discovery service: {:?}", err );
        return ExitCode::FAILURE;
      }
    };

  app::App::new( discovery_rx ).run();
  ExitCode::SUCCESS
}