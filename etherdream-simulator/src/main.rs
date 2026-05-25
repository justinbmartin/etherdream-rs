/// ...
use std::env;
use std::net::{ IpAddr, Ipv4Addr };
use std::process::ExitCode;

use clap::{ Arg, Command };
use etherdream::protocol;
use etherdream_simulator::SimulatorBuilder;

#[tokio::main]
async fn main() -> ExitCode {
  let matches = Command::new( clap::crate_name!() )
    .version( clap::crate_version!() )
    .author( clap::crate_authors!() )
    .about( clap::crate_description!() )
    .args([
      Arg::new( "ip" )
        .help("The IP address to run the simulator with.")
        .default_value( Ipv4Addr::LOCALHOST.to_string() )
        .value_parser( clap::value_parser!( Ipv4Addr ) ),
      Arg::new( "port" )
        .help("The port to run the simulator with.")
        .default_value( protocol::CLIENT_PORT.to_string() )
        .value_parser( clap::value_parser!( u16 ) )
    ])
    .get_matches();

  let ip_addr = matches.get_one::<Ipv4Addr>( "ip" ).unwrap();
  let port = matches.get_one::<u16>( "port" ).unwrap();

  let simulator_builder = SimulatorBuilder::new()
    .ip_addr( IpAddr::V4( *ip_addr ) )
    .port( *port );

  match simulator_builder.start().await {
    Ok( simulator ) => {
      println!( "Listening on: {}", simulator.address() );
      simulator.stop().await;
    },
    Err( err ) => eprintln!( "Failed to start simulator: {}", err )
  }

  ExitCode::SUCCESS
}