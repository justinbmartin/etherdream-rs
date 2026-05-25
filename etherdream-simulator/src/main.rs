//! Simulator command-line application.
use std::env;
use std::net::{ IpAddr, Ipv4Addr, SocketAddr };

use clap::{ Arg, Command };
use etherdream::protocol;
use etherdream_simulator as simulator;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(),String> {
  let matches = Command::new( clap::crate_name!() )
    .version( clap::crate_version!() )
    .author( clap::crate_authors!() )
    .about( clap::crate_description!() )
    .args([
      Arg::new( "host" )
        .default_value( Ipv4Addr::LOCALHOST.to_string() )
        .help( "The IPv4 address the simulator will communicate on." )
        .long( "host" )
        .value_parser( clap::value_parser!( Ipv4Addr ) ),
      Arg::new( "port" )
        .default_value( protocol::CLIENT_PORT.to_string() )
        .help( "The port that the simulator will communicate on." )
        .long( "port" )
        .value_parser( clap::value_parser!( u16 ) ),
      Arg::new( "capacity" )
        .default_value( simulator::DEFAULT_POINT_BUFFER_CAPACITY.to_string() )
        .help( "The capacity of the simulator's point buffer." )
        .long( "capacity" )
        .value_parser( clap::value_parser!( u16 ) )
    ])
    .get_matches();

  let builder = simulator::Builder::new()
    .address( SocketAddr::new(
      IpAddr::V4( *matches.get_one::<Ipv4Addr>( "host" ).unwrap() ),
      *matches.get_one::<u16>( "port" ).unwrap() ) )
    .capacity( *matches.get_one::<u16>( "capacity" ).unwrap() );

  match builder.start().await {
    Ok( simulator ) => {
      println!( "> Etherdream simulator address: {}", simulator.address() );
      println!( "> Use Ctrl+C to exit." );

      if let Err( _ ) = signal::ctrl_c().await {
        return Err( "Failed to setup ctrl+c handling, aborting...".to_owned() );
      }
    },
    Err( err ) => {
      return Err( format!( "Failed to start simulator: {}", err ) );
    }
  }

  Ok( () )
}