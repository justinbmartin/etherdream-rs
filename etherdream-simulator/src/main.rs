/// ...
use std::process::ExitCode;

use etherdream_simulator::Simulator;

#[tokio::main]
async fn main() -> ExitCode {
  match Simulator::start().await {
    Ok( simulator ) => {
      println!( "Listening on: {}", simulator.address() );
      simulator.stop().await;
    },
    Err( err ) => eprintln!( "Failed to start simulator: {}", err )
  }

  ExitCode::SUCCESS
}