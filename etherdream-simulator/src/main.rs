/// ...
use std::process::ExitCode;

use etherdream_simulator::Simulator;

#[tokio::main]
async fn main() -> ExitCode {
  match Simulator::start().await {
    Ok( simulator ) => println!( "Listening on: {}", simulator.address() ),
    Err( err ) => eprintln!( "Failed to start simulator: {}", err )
  }

  ExitCode::SUCCESS
}