use std::collections::HashMap;
use std::io::{ stdin, stdout, Write };
use std::net::SocketAddr;
use std::sync::Arc;

use clap::{ Parser, Subcommand };
use parking_lot::RwLock;

use etherdream::{ Client, Device, discovery };

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  CLI

/// Discover and manage Etherdream devices.
#[derive( Debug, Parser )]
#[command( multicall=true )]
struct Cli {
  #[command( subcommand )]
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

  /// Print details about a discovered device by index.
  #[clap( alias = "c" )]
  Connect {
    /// Index of device to connect to.
    index: usize,
  },

  /// Exit the application.
  Exit,
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Main

struct State {
  // The current client that is active
  current_client: Option<usize>,

  // A map of actively running Etherdream clients
  clients: HashMap<usize,Client>,

  // A list of all discovered Etherdream devices (using a vec to allow accessing by index)
  devices: Vec<( SocketAddr, Device )>
}

#[tokio::main]
async fn main() {
  let state = Arc::new( RwLock::new( State{ clients: HashMap::new(), current_client: None, devices: Vec::new() }));
  let mut input = String::new();

  // Start the Etherdream discovery service
  tokio::task::spawn({
    let state = state.clone();

    async move {
      return discovery::Server::new()
        .serve( move | address, device |{
          state.write().devices.push( ( address, device ) );
        })
        .await;
    }
  });

  // Start the REPL (via a naive loop)
  loop {
    match process_command( &mut input, &state ).await {
      Ok( quit ) => if quit { break; },
      Err( msg ) => println!( "{msg}" )
    }
  }
}

async fn process_command( input: &mut String, state: &Arc<RwLock<State>> ) -> Result<bool,String> {
  input.clear();

  // Print console prefix (one of "> " or "[<device index>] >")
  let prefix = if let Some( index ) = state.read().current_client { format!( "[{}]", index ) } else { String::from( "" ) };
  print!( "{}> ", prefix );
  
  // Read and parse console input
  stdout().flush().map_err( |e|{ format!( "Failed to flush output: {}", e.to_string() ) })?;
  stdin().read_line( input ).map_err( |e| format!( "Failed to read input: {}", e.to_string() ))?;
  
  let args = shlex::split( input.trim() ).ok_or( "Failed to read input." )?;
  let cli = Cli::try_parse_from( args ).map_err( |e| e.to_string() )?;

  // Execute the user-provided command
  match cli.command {
    Commands::Connect{ index } => {
      if state.read().clients.contains_key( &index ) {
        state.write().current_client = Some( index );
        Ok( false )

      } else {
        match state.read().devices.get( index ) {
          Some( ( address, _ ) ) => {
            match Client::start( *address, |_, _, _|{  }).await {
              Ok( client ) => {
                let mut state = state.write();
                state.clients.insert( index, client );
                state.current_client = Some( index );
    
                return Ok( false );
              }

              Err( msg ) => {
                return Err( String::from( format!( "Failed to connect to Etherdream DAC: {}", msg ) ) ); 
              }
            }
          },

          None => { 
            return Err( String::from( "(does not exist)" ) ); 
          }
        }
      }
    },

    Commands::Device{ index } => {
      match state.read().devices.get( index ) {
        Some(( address, device )) => {
          println!( "Address = {address}\n" );
          println!( "{device}" );
          return Ok(false);
        },
        None => { 
          return Err( String::from( "(does not exist)" ) ); 
        }
      }
    }

    Commands::List => {
      let devices = &state.read().devices;

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
