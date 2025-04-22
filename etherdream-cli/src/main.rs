use std::io::{ stdin, stdout, Write };
use std::net::SocketAddr;
use std::sync::Arc;

use clap::{ Parser, Subcommand };
use parking_lot::RwLock;

use etherdream::{ Device, discovery };

type Devices = Arc<RwLock<Vec<( SocketAddr, Device )>>>;


// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  CLI

/// Discover and manage Etherdream devices.
#[derive( Debug, Parser )]
#[command( multicall=true )]
struct Cli {
  #[command(subcommand)]
  command: Commands,
}

#[derive( Debug, Subcommand )]
enum Commands {
  /// List discovered devices.
  #[clap( alias = "ls" )]
  List,

  /// Print details about a discovered device by index.
  #[clap( alias = "d" )]
  Device {
    /// Index of device to view.
    index: usize,
  },

  /// Exit the application.
  Exit,
}


// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Main

#[tokio::main]
async fn main() {
  let devices: Devices = Arc::new( RwLock::new( Vec::new() ) );
  let mut input = String::new();

  // Start the discovery service
  tokio::task::spawn({
    let devices = devices.clone();

    async move {
      return discovery::Server::new()
        .serve( move | address, device |{
          devices.write().push( ( *address, *device ) );
        })
        .await;
    }
  });

  // REPL
  loop {
    match process_command_from_input( &mut input, &devices ) {
      Ok( quit ) => {
        if quit { break; }
      }
      
      Err( msg ) => {
        println!( "{msg}" );
      }
    }
  }
}


// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Support

fn process_command_from_input( input: &mut String, devices: &Devices ) -> Result<bool,String> {
  input.clear();

  // Read input
  print!( "> " );
  stdout().flush().map_err( |e|{ format!( "Failed to flush output: {}", e.to_string() ) })?;
  stdin().read_line( input ).map_err( |e| format!( "Failed to read input: {}", e.to_string() ))?;
  
  // Parse input
  let args = shlex::split( input.trim() ).ok_or( "Failed to read input." )?;
  let cli = Cli::try_parse_from( args ).map_err( |e| e.to_string() )?;

  // Handle command
  match cli.command {
    Commands::Device{ index } => {
      return match devices.read().get( index ) {
        Some(( address, device )) => {
          println!( "Address = {address}\n" );
          println!( "{device}" );
          Ok(false)
        },
        None => { return Err( String::from( "(does not exist)" ) ); }
      }
    }

    Commands::List => {
      let devices = devices.read();

      if devices.is_empty() {
        println!( "(no devices)" );
      } else {
        for ( index, ( address, device ) ) in devices.iter().enumerate() {
          println!( "  [{}] {} (MAC: {})", index, address, device.mac_address() );
        }
      }

      return Ok( false );
    }

    Commands::Exit => {
      return Ok( true );
    }
  }
}
