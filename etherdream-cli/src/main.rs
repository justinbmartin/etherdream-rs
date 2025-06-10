use std::collections::HashMap;
use std::io::{ self, Write };
use std::net::IpAddr;
use std::sync::Arc;

use clap;
use parking_lot::RwLock;

use etherdream::{ self, discovery };

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Main

type StateRef = Arc<RwLock<State>>;

struct State {
  // The current client that is active
  current_client: Option<usize>,

  // A map of actively running Etherdream clients
  clients: HashMap<usize,etherdream::Client>,

  // A list of all discovered Etherdream devices (using a vec to allow accessing by index)
  devices: Vec<( IpAddr, etherdream::Device )>
}

#[tokio::main]
async fn main() {
  let mut input = String::new();
  let state = Arc::new( RwLock::new( State{ clients: HashMap::new(), current_client: None, devices: Vec::new() }));

  let mut cli = clap::Command::new( "Etherdream" )
    .about( "Discover and manage Etherdream devices." )
    .multicall( true )
    .subcommand_required( true )
    .subcommands([
      clap::Command::new( "list" )
        .about( "List discovered Etherdream devices." )
        .visible_alias( "ls" ),
      clap::Command::new( "info" )
        .about( "Prints details about a discovered device at `index`" )
        .arg( clap::Arg::new( "index" ).required( true ).value_parser( clap::value_parser!( usize ) ) ),
      clap::Command::new( "connect" )
        .about( "Connects to a discovered Etherdream device at the provided `index`." )
        .arg( clap::Arg::new( "index" ).required( true ).value_parser( clap::value_parser!( usize ) ) ),
      clap::Command::new( "disconnect" )
        .about( "Disconnects from the currently active Etherdream device." ),
      clap::Command::new( "ping" )
        .about( "Pings the currently active Etherdream device and prints the active device state." ),
      clap::Command::new( "exit" )
    ]);

  // Start the Etherdream discovery service. All discovered devices will be
  // stored in `state.devices`.
  tokio::task::spawn({
    let state = state.clone();

    async move {
      return discovery::Server::new( move | ip, device | { state.write().devices.push( ( ip, device ) ); })
        .serve()
        .await;
    }
  });

  // REPL
  loop {
    // Print console prefix, one of "> " or "[<device index>] >"
    let prefix = if let Some( index ) = state.read().current_client { format!( "[{}] ", index ) } else { String::from( "" ) };
    print!( "{}> ", prefix );
  
    // Read and parse console input
    input.clear();
    let _ = io::stdout().flush();
    let _ = io::stdin().read_line( &mut input );
  
    if let Some( args ) = shlex::split( input.trim() ) {
      match cli.try_get_matches_from_mut( args ) {
        Ok( matches ) =>
          match matches.subcommand() {
            Some(( "connect", args )) => do_connect( &state, *args.get_one::<usize>( "index" ).unwrap() ).await,
            Some(( "disconnect", _ )) => do_disconnect( &state ).await,
            Some(( "exit", _ )) => break,
            Some(( "info", args )) => do_print_device_info( &state, *args.get_one::<usize>( "index" ).unwrap() ),
            Some(( "list", _ )) => do_list_devices( &state ),
            Some(( "ping", _ )) => do_ping_current_device( &state ).await,
            _ => {}
          },
        Err( err ) => 
          { println!( "{err}" ); }
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
    for ( index, ( ip, device ) ) in state.devices.iter().enumerate() {
      println!( "  [{}] {} (MAC: {})", index, ip, device.mac_address );
    }
  }
}

async fn do_connect( state: &StateRef, device_index: usize ) {
  // Define client command callback
  let callback_fn = {
    move | _control, _command, _points_remaining | { 
      //println!( "COMMAND RECEIVED: {:?}", command );
    }
  };

  let mut state = state.write();

  // If client already exists, set it as the current device
  if state.clients.contains_key( &device_index ) {
    state.current_client = Some( device_index );
  } 
  
  // Create a new client for the device at `device_index`
  else {
    match state.devices.get( device_index ) {
      Some( &( ip, _ ) ) => {
        match etherdream::Client::connect( ip, callback_fn ).await {
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
    Some(( ip, device )) => {
      println!( "IP = {ip}\n" );
      println!( "{device}" );
    },
    None => { 
      println!( "(does not exist)" ); 
    }
  }
}

async fn do_ping_current_device( state: &StateRef ) {
  let state = state.read();

  if let Some( client_index ) = state.current_client {
    let client = state.clients.get( &client_index ).unwrap();
    
    match client.ping().await {
      Ok( state ) => {
        println!( "Acknowledged: yes" );
        println!( "\n{state}" );
      },
      _ => println!( "Ping error..." )
    }
  } else {
    println!( "(no device connected)" );
  }
}
