//! CLI tool to discover, connect and test Etherdream DAC's.
use std::collections::HashMap;
use std::io::{ self, Write };
use std::sync::Arc;

use clap;
use etherdream;
use tokio::sync::RwLock;

#[derive( Clone, Copy, Default )]
struct Point {
  x: i16,
  y: i16,
  r: u16,
  g: u16,
  b: u16
}

impl etherdream::Point for Point {
  fn for_etherdream( &self ) -> ( i16, i16, u16, u16, u16 ) {
    ( self.x, self.y, self.r, self.g, self.b )
  }
}

struct App {
  // The REPL's active Etherdream client, by index in `clients`
  current_client: Option<usize>,

  // A map of actively connected Etherdream clients, indexed by a constant id
  clients: HashMap<usize,etherdream::Client<Point>>,

  // A list of all discovered Etherdream devices
  devices: Arc<RwLock<Vec<etherdream::Device>>>
}

impl App {
  async fn do_list_devices( &self ) {
    let devices = self.devices.read().await;

    if devices.is_empty() {
      println!( "(no devices)" );
    } else {
      for ( index, device ) in devices.iter().enumerate() {
        println!( "  [{}] {} (MAC: {})", index, device.address().ip(), device.mac_address() );
      }
    }
  }

  async fn do_connect( &mut self, device_index: usize ) {
    // If client already exists, set it as the current device. Else, create a
    // new client.
    if self.clients.contains_key( &device_index ) {
      self.current_client = Some( device_index );
    } else {
      let devices = self.devices.read().await;

      match devices.get( device_index ) {
        Some( &device ) => {
          match etherdream::ClientBuilder::new( device ).connect().await {
            Ok( client ) => {
              self.clients.insert( device_index, client );
              self.current_client = Some( device_index );
            }

            Err( msg ) => {
              println!( "Failed to connect to Etherdream DAC: {:?}", msg );
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
    let devices = self.devices.read().await;

    match devices.get( device_index ) {
      Some( device ) => {
        println!( "IP = {}\n", device.address().ip() );
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

  async fn do_play_current_device( &mut self, count: usize ) {
    if let Some( client_index ) = self.current_client {
      let client = self.clients.get_mut( &client_index ).unwrap();

      // 1. reset the client
      // 2. client player

      println!( "pushing points..." );
      for _ in 0..count {
        client.push_point( Point{
          x: 0,
          y: 0,
          r: u16::MAX,
          g: u16::MAX,
          b: u16::MAX
        });
      }

      client.flush_points().await;

      // Start processing point data
      println!( "starting..." );
      let _ = client.start( 1000 ).await;

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

  async fn do_print_status_current_device( &self ) {
    if let Some( client_index ) = self.current_client {
      let client = self.clients.get( &client_index ).unwrap();
      let mut state = etherdream::device::State::default();
      client.copy_state( &mut state ).await;

      print_device_state( &state );
    } else {
      println!( "(no device connected)" );
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Main

#[tokio::main]
async fn main() {
  let mut input = String::new();

  let mut app = App{
    clients: HashMap::new(),
    current_client: None,
    devices: Arc::new( RwLock::new( Vec::new() ) )
  };

  // Start the Etherdream discovery service. Each discovered device:
  // - Will be received async via `rx`.
  // - Will be persisted in `app.devices`.
  tokio::spawn({
    let devices = app.devices.clone();

    async move {
      let ( tx, mut rx ) = tokio::sync::mpsc::channel( 16 );
      let _ = etherdream::Discovery::serve( tx ).await;

      while let Some( device ) = rx.recv().await {
        devices.write().await.push( device );
      }
    }
  });

  // Define the REPL interface
  let mut cli = clap::Command::new( "Etherdream" )
    .about( "Discover and manage Etherdream devices." )
    .disable_help_subcommand( true )
    .multicall( true )
    .subcommand_required( true )
    .subcommands([
      clap::Command::new( "connect" )
        .about( "Connects to a discovered Etherdream device at the provided `index`." )
        .arg( clap::Arg::new( "index" ).required( true ).value_parser( clap::value_parser!( usize ) ) )
        .visible_alias( "cd" ),
      clap::Command::new( "disconnect" )
        .about( "Disconnects from the currently active Etherdream device." ),
      clap::Command::new( "exit" ),
      clap::Command::new( "help" ),
      clap::Command::new( "list" )
        .about( "List discovered Etherdream devices." )
        .visible_alias( "ls" ),
      clap::Command::new( "info" )
        .about( "Prints details about a discovered device at `index`" )
        .arg( clap::Arg::new( "index" ).required( true ).value_parser( clap::value_parser!( usize ) ) ),
      clap::Command::new( "ping" )
        .about( "Pings the currently active Etherdream device and prints the active device state." ),
      clap::Command::new( "play" )
        .about( "Produces `count` number of points into the client's buffer." )
        .arg( clap::Arg::new( "count" ).required( true ).value_parser( clap::value_parser!( usize ) ) ),
      clap::Command::new( "reset" )
        .about( "(tmp) empties the point buffer in the DAC" ),
      clap::Command::new( "status" )
        .about( "(tmp) returns the last reported status from the DAC (non-ping)" )
    ]);

  // Start the REPL
  loop {
    // Print console prefix
    if let Some( index ) = app.current_client {
      print!( "[{}]> ", index )
    } else {
      print!( "> " )
    }

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
            Some(( "help", _ )) => cli.print_help().unwrap(),
            Some(( "info", args )) => app.do_print_device_info( *args.get_one::<usize>( "index" ).unwrap() ).await,
            Some(( "list", _ )) => app.do_list_devices().await,
            Some(( "ping", _ )) => app.do_ping_current_device().await,
            Some(( "play", args )) => app.do_play_current_device( *args.get_one::<usize>( "count" ).unwrap() ).await,
            Some(( "reset", _ )) => app.do_reset_current_device().await,
            Some(( "status", _ )) => app.do_print_status_current_device().await,
            _ => {}
          },
        Err( _ ) => {}
      }
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Helpers

fn print_device( device: &etherdream::Device ) {
  println!( "Device:" );
  println!( "  Buffer capacity = {}", device.buffer_capacity() );
  println!( "  Mac address = {}", device.mac_address() );
  println!( "  Max points per second = {}", device.max_points_per_second() );
  println!( "  Version = hardware: {}; software: {}", device.version().hardware, device.version().software );
  println!( "State:" );

  print_device_state( &device.state() );
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