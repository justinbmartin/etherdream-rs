/// ...
use std::env;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };
use std::process::ExitCode;

use clap::{ Arg, Command };
use etherdream::protocol;
use etherdream_simulator as simulator;

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
        .value_parser( clap::value_parser!( u16 ) ),
      Arg::new( "capacity" )
        .help("The capacity of the point buffer.")
        .default_value( 1024.to_string() )
        .value_parser( clap::value_parser!( u16 ) )
    ])
    .get_matches();

  let builder = simulator::Builder::new()
    .address( SocketAddr::new(
      IpAddr::V4( *matches.get_one::<Ipv4Addr>( "ip" ).unwrap() ),
      *matches.get_one::<u16>( "port" ).unwrap() ) )
    .capacity( *matches.get_one::<u16>( "capacity" ).unwrap() );

  match builder.start().await {
    Ok( simulator ) => {
      println!( "Listening on: {}", simulator.address() );
      simulator.stop().await;
    },
    Err( err ) => eprintln!( "Failed to start simulator: {}", err )
  }

  ExitCode::SUCCESS
}