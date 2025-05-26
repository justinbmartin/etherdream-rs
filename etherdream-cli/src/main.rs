use std::collections::HashMap;
use std::io::{ self, Write };
use std::net::SocketAddr;
use std::sync::Arc;

use clap::{ self, Parser };
use parking_lot::RwLock;

use etherdream;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  CLI

/// Discover and manage Etherdream devices.
#[derive( Debug, clap::Parser )]
#[command( multicall=true )]
struct Cli {
  #[command( subcommand )]
  command: Commands,
}

#[derive( Debug, clap::Subcommand )]
enum Commands {
  /// List discovered devices.
  #[clap( alias = "ls" )]
  List,

  /// Print information about a discovered device by index.
  Info {
    /// Index of device to view.
    index: usize,
  },

  /// Connects to a discovered Etherdream by index.
  Connect {
    /// Index of device to connect to.
    index: usize,
  },

  /// Disconnects from the currently active Etherdream client.
  Disconnect,

  /// Ping the currently selected device and print the current state.
  Ping,

  /// Exit the application.
  Exit,
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Main

type StateRef = Arc<RwLock<State>>;

struct State {
  // The current client that is active
  current_client: Option<usize>,

  // A map of actively running Etherdream clients
  clients: HashMap<usize,etherdream::Client>,

  // A list of all discovered Etherdream devices (using a vec to allow accessing by index)
  devices: Vec<( SocketAddr, etherdream::Device )>
}

#[tokio::main]
async fn main() {
  let mut input = String::new();
  let state = Arc::new( RwLock::new( State{ clients: HashMap::new(), current_client: None, devices: Vec::new() }));
  
  // Start the Etherdream discovery service. All discovered devices will be
  // stored in `state.devices`.
  tokio::task::spawn({
    let state = state.clone();

    async move {
      return etherdream::discovery::Server::new()
        .serve( move | address, device | { state.write().devices.push( ( address, device ) ); })
        .await;
    }
  });

  // Start the REPL
  loop {
    // Print console prefix, one of "> " or "[<device index>] >"
    let prefix = if let Some( index ) = state.read().current_client { format!( "[{}] ", index ) } else { String::from( "" ) };
    print!( "{}> ", prefix );
  
    // Read and parse console input
    input.clear();
    let _ = io::stdout().flush();
    let _ = io::stdin().read_line( &mut input );
  
    if let Some( args ) = shlex::split( input.trim() ) {
      if let Ok( cli ) = Cli::try_parse_from( args ) {

        match cli.command {
          Commands::Connect{ index } => do_connect( &state, index ).await,
          Commands::Disconnect => do_disconnect( &state ).await,
          Commands::Exit => break,
          Commands::Info{ index } => do_print_device_info( &state, index ),
          Commands::List => do_list_devices( &state ),
          Commands::Ping => do_ping_current_device( &state )
        }
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Command Handlers

fn do_list_devices( state: &StateRef ) {
  let state = state.read();

  if state.devices.is_empty() {
    println!( "(no devices)" );
  } else {
    for ( index, ( address, device ) ) in state.devices.iter().enumerate() {
      println!( "  [{}] {} (MAC: {})", index, address, device.mac_address() );
    }
  }
}

async fn do_connect( state: &StateRef, device_index: usize ) {
  let mut state = state.write();

  // Define client command callback
  let callback_fn = move | _control, command, _points_remaining | { 
    println!( "COMMAND RECEIVED: {:?}", command );
  };

  // If client already exists, set it as the current device
  if state.clients.contains_key( &device_index ) {
    state.current_client = Some( device_index );

  } 
  
  // Create a new client for the device at `device_index`
  else {
    match state.devices.get( device_index ) {
      Some( &( address, _ ) ) => {
        match etherdream::Client::connect( address.ip(), callback_fn ).await {
          Ok( client ) => {
            state.clients.insert( device_index, client );
            state.current_client = Some( device_index );
          }

          Err( msg ) => {
            println!( "Failed to connect to Etherdream DAC: {}", msg ); 
          }
        }
      },

      None => { 
        println!( "(does not exist)" ); 
      }
    }
  }
}

async fn do_disconnect( state: &StateRef ) {
  let mut state = state.write();

  if let Some( index ) = state.current_client {
    if let Some( client ) = state.clients.remove( &index ) {
      let _ = client.disconnect().await;
    }
  }
}

fn do_print_device_info( state: &StateRef, device_index: usize ) {
  match state.read().devices.get( device_index ) {
    Some(( address, device )) => {
      println!( "Address = {address}\n" );
      println!( "{device}" );
    },
    None => { 
      println!( "(does not exist)" ); 
    }
  }
}

fn do_ping_current_device( state: &StateRef ) {
  let state = state.read();

  if let Some( client_index ) = state.current_client {
    let client = state.clients.get( &client_index ).unwrap();
    let _ = client.ping();
    
    //

  } else {
    println!( "(no device connected)" );
  }
}
