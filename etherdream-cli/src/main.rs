use std::collections::HashMap;
use std::io::{ self, Write };
use std::net::SocketAddr;
use std::sync::{ Arc, RwLock };

use clap;

use etherdream;

struct App {
  // The current client that is active
  current_client: Option<usize>,

  // A map of actively running Etherdream clients
  clients: HashMap<usize,etherdream::Client>,

  // A list of all discovered Etherdream devices (using a vec to allow accessing by index)
  devices: Arc<RwLock<Vec<( SocketAddr, etherdream::Device )>>>
}

impl App {
  async fn do_list_devices( &self ) {
    let devices = self.devices.read().unwrap();

    if devices.is_empty() {
      println!( "(no devices)" );
    } else {
      for ( index, ( ip, device ) ) in devices.iter().enumerate() {
        println!( "  [{}] {} (MAC: {})", index, ip, device.mac_address );
      }
    }
  }

  async fn do_connect( &mut self, device_index: usize ) {

    // If client already exists, set it as the current device
    if self.clients.contains_key( &device_index ) {
      self.current_client = Some( device_index );
    }

    // Create a new client for the device at `device_index`
    else {
      let devices = self.devices.read().unwrap();

      match devices.get( device_index ) {
        Some( &( socket, _device ) ) => {
          match etherdream::Client::new( socket ).await {
            Ok( client ) => {
              self.clients.insert( device_index, client );
              self.current_client = Some( device_index );
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

  async fn do_disconnect( &mut self ) {
    if let Some( index ) = self.current_client {
      if let Some( client ) = self.clients.remove( &index ) {
        let _ = client.disconnect().await;
      }
    }
  }

  async fn do_print_device_info( &self, device_index: usize ) {
    let devices = self.devices.read().unwrap();

    match devices.get( device_index ) {
      Some(( ip, device )) => {
        println!( "IP = {ip}\n" );
        print_device( &device );
      },
      None => {
        println!( "(does not exist)" );
      }
    }
  }

  async fn do_ping_current_device( &mut self ) {
    if let Some( client_index ) = self.current_client {
      let client = self.clients.get_mut( &client_index ).unwrap();

      match client.ping().await {
        Ok( state ) => {
          println!( "Acknowledged: yes" );
          print_device_state( &state );
        },
        _ => println!( "Ping error..." )
      }
    } else {
      println!( "(no device connected)" );
    }
  }

  async fn do_play_current_device( &mut self ) {
    if let Some( client_index ) = self.current_client {
      let client = self.clients.get_mut( &client_index ).unwrap();

      for _ in 0..3000 {
        client.push_point( 0, 0, u16::MAX, u16::MAX, u16::MAX );
      }

      // Temporarily sleep for a moment...
      //tokio::time::sleep( tokio::time::Duration::from_secs( 1 ) ).await;

      // Start...
      client.start( 1500 );

    } else {
      println!( "(no device connected)" );
    }
  }

  async fn do_reset_current_device( &mut self ) {
    if let Some( client_index ) = self.current_client {
      let client = self.clients.get_mut( &client_index ).unwrap();
      let _ = client.reset().await;
    } else {
      println!( "(no device connected)" );
    }
  }

  fn do_print_status_current_device( &self ) {
    if let Some( client_index ) = self.current_client {
      let client = self.clients.get( &client_index ).unwrap();
      print_device_state( &client.state() );
    } else {
      println!( "(no device connected)" );
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Main

#[tokio::main]
async fn main() {
  let mut input = String::new();
  let mut app = App{ clients: HashMap::new(), current_client: None, devices: Arc::new( RwLock::new( Vec::new() ) ) };

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
        .arg( clap::Arg::new( "index" ).required( true ).value_parser( clap::value_parser!( usize ) ) )
        .visible_alias( "cd" ),
      clap::Command::new( "disconnect" )
        .about( "Disconnects from the currently active Etherdream device." ),
      clap::Command::new( "ping" )
        .about( "Pings the currently active Etherdream device and prints the active device state." ),
      clap::Command::new( "play" )
        .about( "..." ),
      clap::Command::new( "reset" )
        .about( "(tmp) empties the point buffer in the DAC" ),
      clap::Command::new( "status" )
        .about( "(tmp) returns the last reported status from the DAC (non-ping)" ),
      clap::Command::new( "exit" )
    ]);

  // Start the Etherdream discovery service. All discovered devices will be stored in
  // `state.devices`.
  tokio::task::spawn({
    let devices = app.devices.clone();

    async move {
      etherdream::Discovery::new(
        move | socket, device | {
          // Safety: Unwrap is safe, as this is the only write invocation in discovery thread.
          devices.write().unwrap().push( ( socket, device ) );
        })
        .serve()
        .await
    }
  });

  // REPL
  loop {
    // Print console prefix, one of "> " or "[<device index>] >"
    let prefix = if let Some( index ) = app.current_client { format!( "[{}] ", index ) } else { String::from( "" ) };
    print!( "{}> ", prefix );
  
    // Read and parse console input
    input.clear();
    let _ = io::stdout().flush();
    let _ = io::stdin().read_line( &mut input );
  
    if let Some( args ) = shlex::split( input.trim() ) {
      match cli.try_get_matches_from_mut( args ) {
        Ok( matches ) =>
          match matches.subcommand() {
            Some(( "connect", args )) => app.do_connect( *args.get_one::<usize>( "index" ).unwrap() ).await,
            Some(( "disconnect", _ )) => app.do_disconnect().await,
            Some(( "exit", _ )) => break,
            Some(( "info", args )) => app.do_print_device_info( *args.get_one::<usize>( "index" ).unwrap() ).await,
            Some(( "list", _ )) => app.do_list_devices().await,
            Some(( "ping", _ )) => app.do_ping_current_device().await,
            Some(( "status", _ )) => app.do_print_status_current_device(),
            Some(( "play", _ )) => app.do_play_current_device().await,
            Some(( "reset", _ )) => app.do_reset_current_device().await,
            _ => {}
          },
        Err( err ) => 
          { println!( "{err}" ); }
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Helpers

fn print_device( device: &etherdream::Device ) {
  println!( "Device:" );
  println!( "  Buffer capacity = {}", device.buffer_capacity );
  println!( "  Mac address = {}", device.mac_address );
  println!( "  Max points per second = {}", device.max_points_per_second );
  println!( "  Version = hardware: {}; software: {}", device.version.hardware, device.version.software );
  println!( "State:" );

  print_device_state( &device.state );
}

fn print_device_state( state: &etherdream::device::State ) {
  let shutter_state = 1 & ( state.playback_flags >> 0 );
  let underflow_state = 1 & ( state.playback_flags >> 1 );
  let e_stop_state = 1 & ( state.playback_flags >> 2 );

  println!( "  Light engine = {:?}", state.light_engine_state );
  println!( "  Light engine flag = {:?}", state.light_engine_flags );
  println!( "  Playback = {:?}", state.playback_state );
  println!( "    Shutter = {:?}", shutter_state );
  println!( "    Underflow = {:?}", underflow_state );
  println!( "    E-stop = {:?}", e_stop_state );
  println!( "  Source = {:?}", state.source );
  println!( "  Points buffered = {:?}", state.points_buffered );
  println!( "  Points per second = {:?}", state.points_per_second );
  println!( "  Points lifetime = {:?}", state.points_lifetime );
}