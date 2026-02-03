//! CLI tool to discover, connect and test Etherdream DAC's.
mod generators;

use std::collections::HashMap;
use std::io::{ self, Write };
use std::sync::Arc;

use clap;
use etherdream::generator;
use tokio::sync::{ RwLock, RwLockReadGuard };
use tokio::task::JoinHandle;

use generators::GeneratorId;

const TEXT_NO_ACTIVE_DEVICE: &str = "(no active device)";
const TEXT_NOT_FOUND: &str = "(not found)";

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Main

#[tokio::main]
async fn main() {
  let mut app = App::new().await;
  let mut input = String::new();

  // Define the REPL interface
  let mut cli = clap::Command::new( "Etherdream" )
    .about( "Discover and manage Etherdream devices." )
    .disable_help_subcommand( true )
    .multicall( true )
    .subcommand_required( true )
    .subcommands([
      clap::Command::new( "connect" )
        .about( "Connects to a discovered Etherdream device at the provided `index`." )
        .arg( clap::Arg::new( "index" )
          .required( true )
          .value_parser( clap::value_parser!( usize ) ) ),
      clap::Command::new( "exit" ),
      clap::Command::new( "help" ),
      clap::Command::new( "info" )
        .about( "Prints info about the currently connected device." ),
      clap::Command::new( "list" )
        .about( "List discovered Etherdream devices." )
        .visible_alias( "ls" ),
      clap::Command::new( "ping" )
        .about( "Pings the currently active Etherdream device and prints the active device state." ),
      clap::Command::new( "play" )
        .about( "Starts a generator." )
        .arg( clap::Arg::new( "generator" )
          .default_value( "xs" )
          .value_parser([ "xs" ])
          .help( "Must be one of: xs" ) ),
      clap::Command::new( "stop" )
        .about( "Stops an actively running generator." )
    ]);

  loop {
    // Print console prefix
    if let Some( index ) = app.current_index() {
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
            Some(( "connect", args )) => {
              if let Err( err ) = app.connect( *args.get_one::<usize>( "index" ).unwrap() ).await {
                print_err( err, "Failed to connect" )
              }
            },
            Some(( "exit", _ )) => {
              break
            },
            Some(( "help", _ )) => {
              cli.print_help().unwrap()
            },
            Some(( "info", _ )) => {
              if let Some( index ) = app.current_index() {
                let device_info = app.device_info( index ).await.unwrap();

                println!( "Device:" );
                println!( "  Buffer capacity = {}", device_info.buffer_capacity() );
                println!( "  Mac address = {}", device_info.mac_address() );
                println!( "  Max points per second = {}", device_info.max_points_per_second() );
                println!( "  Version = hardware: {}; software: {}", device_info.version().hardware, device_info.version().software );
              } else {
                println!( "{TEXT_NO_ACTIVE_DEVICE}" );
              }
            }
            Some(( "list", _ )) => {
              let devices = app.devices().await;

              if devices.is_empty() {
                println!( "(no devices)" );
              } else {
                for ( index, device ) in devices.iter().enumerate() {
                  println!( "  [{}] {} (MAC: {})", index, device.address().ip(), device.mac_address() );
                }
              }
            },
            Some(( "ping", _ )) => {
              if let Some( index ) = app.current_index() {
                match app.ping( index ).await {
                  Ok( state ) => {
                    println!( "State:" );
                    println!( "  Light engine = {:?}", state.light_engine_state() );
                    println!( "  Playback = {:?}", state.playback_state() );
                    println!( "    Shutter = {:?}", state.is_shutter_open() );
                    println!( "    Underflow = {:?}", state.is_underflow() );
                    println!( "    E-stop = {:?}", state.is_e_stop() );
                    println!( "  Source = {:?}", state.source() );
                    println!( "  Points buffered = {:?}", state.points_buffered() );
                    println!( "  Points per second = {:?}", state.points_per_second() );
                    println!( "  Points lifetime = {:?}", state.points_lifetime() );
                  },
                  Err( err ) => print_err( err, "Failed to ping" )
                }
              } else {
                println!( "{TEXT_NO_ACTIVE_DEVICE}" );
              }
            },
            Some(( "play", args )) => {
              if let Some( index ) = app.current_index() {
                if let Some( generator_id ) = GeneratorId::from_str( args.get_one::<String>( "generator" ).unwrap().as_str() ) {
                  if let Err( err ) = app.play( index, generator_id ).await {
                    print_err( err, "Failed to play" )
                  }
                } else {
                  println!( "(unknown generator)" );
                }
              } else {
                println!( "{TEXT_NO_ACTIVE_DEVICE}" );
              }
            },
            Some(( "stop", _ )) => {
              if let Some( index ) = app.current_index() {
                if let Err( err ) = app.stop( index ).await {
                  print_err( err, "Failed to stop" )
                }
              } else {
                println!( "{TEXT_NO_ACTIVE_DEVICE}" );
              }
            },
            _ => {
              /* unrecognized subcommand, no-op */
            }
          },
        Err( _ ) => {}
      }
    }
  }
}

fn print_err( err: Error, prefix: &str ) {
  match err {
    Error::NotFound => println!( "{TEXT_NOT_FOUND}" ),
    Error::Internal( msg ) => eprintln!( "{prefix}: {msg}" )
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Error

pub enum Error {
  Internal( String ),
  NotFound
}

impl From<generator::Error> for Error {
  fn from( err: generator::Error ) -> Self {
    Self::Internal( format!( "Generator error: {:?}", err ) )
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  App

pub struct App {
  // The currently active generator
  current_index: Option<usize>,
  // A list of all discovered Etherdream devices
  discovered_devices: Arc<RwLock<Vec<etherdream::DeviceInfo>>>,
  // The handle to the discovery server
  _discovery_handle: JoinHandle<()>,
  // A map of running generators, indexed by an id
  generators: HashMap<usize,generator::Generator>,
}

impl App {
  pub async fn new() -> Self {
    let discovered_devices = Arc::new( RwLock::new( Vec::new() ) );

    // Start the Etherdream discovery service.
    let discovery_handle = tokio::spawn({
      let discovered_devices = discovered_devices.clone();

      async move {
        let ( tx, mut rx ) = tokio::sync::mpsc::channel( 16 );
        let _ = etherdream::Discovery::serve( tx ).await;

        while let Some( discovered_device ) = rx.recv().await {
          discovered_devices.write().await.push( discovered_device.info().clone() );
        }
      }
    });

    App{
      current_index: None,
      discovered_devices,
      _discovery_handle: discovery_handle,
      generators: HashMap::new()
    }
  }

  /// Returns the currently active device id
  pub fn current_index( &self ) -> Option<usize> {
    self.current_index
  }

  /// Returns an iterator of all discovered devices.
  pub async fn devices( &self ) -> RwLockReadGuard<'_, Vec<etherdream::DeviceInfo>> {
    self.discovered_devices.read().await
  }

  /// Returns back the `DeviceInfo` at `index`.
  pub async fn device_info( &self, index: usize ) -> Option<etherdream::DeviceInfo> {
    self.discovered_devices.read().await.get( index ).copied()
  }

  /// Connects to the discovered device at `index`
  pub async fn connect( &mut self, index: usize ) -> Result<(),Error> {
    // Generator already exists.
    if self.generators.contains_key( &index ) {
      self.current_index = Some( index );
      return Ok( () );
    }

    // Connect to device using `DeviceInfo` at discovered `index`
    match self.discovered_devices.read().await.get( index ) {
      Some( &device_info ) => {
        match etherdream::connect( device_info ).await {
          Ok( client ) => {
            let generator = etherdream::generator( client, Box::new( generators::Noop::new() ) );
            self.generators.insert( index, generator );
            self.current_index = Some( index );
            Ok( () )
          }

          Err( err ) => {
            Err( Error::Internal( format!( "{:?}", err ) ) )
          }
        }
      },

      None => {
        Err( Error::NotFound )
      }
    }
  }

  /// Send a ping to the device associated with the generator at `index`.
  pub async fn ping( &mut self, index: usize ) -> Result<etherdream::State,Error> {
    if let Some( generator ) = self.generators.get_mut( &index ) {
      match generator.ping().await {
        Ok( state ) => Ok( state ),
        Err( err ) => Err( Error::Internal( format!( "{:?}", err ) ) )
      }
    } else {
      Err( Error::NotFound )
    }
  }

  pub async fn play( &mut self, index: usize, generator_id: GeneratorId ) -> Result<(),Error> {
    if let Some( generator ) = self.generators.remove( &index ) {
      let client = generator.into_client().await?;

      match generator_id {
        GeneratorId::Xs => {
          let mut generator = etherdream::generator( client, Box::new( generators::Xs::new() ) );
          generator.start().await;
          self.generators.insert( index, generator );
          Ok( () )
        }
      }
    } else {
      Err( Error::NotFound )
    }
  }

  pub async fn stop( &mut self, index: usize ) -> Result<(),Error> {
    if let Some( generator ) = self.generators.remove( &index ) {
      let mut generator = etherdream::generator(
        generator.into_client().await?,
        Box::new( generators::Noop::new() ) );
      generator.start().await;
      self.generators.insert( index, generator );
      Ok( () )
    } else {
      // TODO
      Err( Error::Internal( "Generator not found".to_string() ) )
    }
  }
}