//! CLI tool to discover, connect and test Etherdream DAC's.
mod executors;

use std::collections::HashMap;
use std::io::{ self, Write };
use std::sync::Arc;

use clap;
use etherdream::generator;
use tokio::sync::RwLock;

const CONSOLE_PREFIX: &str = "> ";
const TEXT_CLIENT_ERROR: &str = "client error: ";
const TEXT_NO_ACTIVE_DEVICE: &str = "(no active device)";
const TEXT_NOT_FOUND: &str = "(not found)";

type DeviceInfosRef = Arc<RwLock<Vec<etherdream::DeviceInfo>>>;
type GeneratorMap = HashMap<usize,etherdream::Generator>;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Main

#[tokio::main]
async fn main() {
  let mut current_index = None::<usize>;
  let device_infos: DeviceInfosRef = Arc::new( RwLock::new( Vec::new() ) );
  let mut generators = GeneratorMap::new();
  let mut input = String::new();

  // Start the Etherdream discovery service
  let ( device_info_tx, mut device_info_rx ) = tokio::sync::mpsc::channel( 16 );

  let discovery_server =
    match etherdream::discover( device_info_tx ).await {
      Ok( server ) => server,
      Err( err ) => {
        println!( "(discovery error: {:?})", err );
        return;
      }
    };

  // Start a task to receive discovered devices and add them to `device_infos`
  let device_info_handle = tokio::spawn({
    let device_infos = device_infos.clone();

    async move {
      while let Some( discovered_device_info ) = device_info_rx.recv().await {
        device_infos.write().await.push( discovered_device_info.info().clone() );
      }
    }
  });

  // Define the REPL interface
  let mut cli = clap::Command::new( "Etherdream" )
    .about( "Discover and test Etherdream devices." )
    .disable_help_flag( true )
    .disable_help_subcommand( true )
    .multicall( true )
    .subcommand_required( true )
    .subcommands([
      clap::Command::new( "connect" )
        .about( "Connects to a discovered device at the index from `list`. This device becomes the currently active device." )
        .arg( clap::Arg::new( "index" )
          .required( true )
          .value_parser( clap::value_parser!( usize ) ) ),
      clap::Command::new( "exit" )
        .about( "Shuts down all generators and closes the console." ),
      clap::Command::new( "help" )
        .about( "Prints command help." ),
      clap::Command::new( "list" )
        .about( "Lists all discovered Etherdream devices." )
        .visible_alias( "ls" ),
      clap::Command::new( "ping" )
        .about( "Pings the currently active device and prints the device state." ),
      clap::Command::new( "play" )
        .about( "Starts a generator for the currently active device. Available generators: demo. [default: demo]" )
        .arg( clap::Arg::new( "generator" )
          .default_value( "demo" )
          .value_parser([ "demo" ]) ),
      clap::Command::new( "stop" )
        .about( "Stops a running generator for the currently active device." )
    ]);

  // Start the REPL
  loop {
    input.clear();

    // Print the console prefix (including `current_index`, if set)
    if let Some( index ) = current_index {
      print!( "[{}]{}", index, CONSOLE_PREFIX )
    } else {
      print!( "{CONSOLE_PREFIX}" );
    }

    // Read CLI input
    let _ = io::stdout().flush();
    let _ = io::stdin().read_line( &mut input );
    let Some( args ) = shlex::split( input.trim() ) else { continue };

    // Handle CLI command
    match cli.try_get_matches_from_mut( args ) {
      Ok( matches ) => {
        let console_out =
          match matches.subcommand() {
            Some( ( "connect", args ) ) => {
              // SAFETY: Unwrap is safe since `index` is a required argument
              let index = args.get_one::<usize>( "index" ).unwrap();
              do_connect( &device_infos, &mut generators, &mut current_index, *index ).await
            }
            Some( ( "exit", _ ) ) => break,
            Some( ( "help", _ ) ) => { let _ = cli.print_long_help(); None }
            Some( ( "list", _ ) ) => do_list( &device_infos ).await,
            Some( ( "ping", _ ) ) => do_ping( &device_infos, &mut generators, current_index ).await,
            Some( ( "play", args ) ) => {
              // SAFETY: Unwrap is safe since `generator` has a default value
              let generator_name = args.get_one::<String>( "generator" ).unwrap();
              do_play( &mut generators, current_index, generator_name ).await
            }
            Some( ( "stop", _ ) ) => do_stop( &mut generators, current_index ).await,
            Some( _ ) | None => Some( "(unknown cmd)".to_owned() )
          };

        // Print any generated console output from commands
        if let Some( msg ) = console_out {
          println!( "{msg}" )
        }
      },

      Err( err ) => {
        match err.kind() {
          clap::error::ErrorKind::MissingSubcommand => {
            // Ignore any missing subcommand errors. An empty command is ok in
            // the REPL.
          },
          _ => println!( "{err}" )
        }
      }
    }
  }

  // Shutdown all generators
  for ( _, generator ) in generators.drain() {
    match generator.into_client().await {
      Ok( client ) => client.disconnect().await,
      Err( _ ) => {}
    }
  }

  // Shutdown the discovery server
  discovery_server.shutdown().await;
  let _ = device_info_handle.await;
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Command Handlers

async fn do_connect(
  device_infos: &DeviceInfosRef,
  generators: &mut GeneratorMap,
  current_index: &mut Option<usize>,
  index: usize
) -> Option<String> {
  // If generator already exists for `index`, assign it as our
  // `current_index` and continue REPL.
  if generators.contains_key( &index ) {
    *current_index = Some( index );
    return None
  }

  // Connect to Etherdream using the `DeviceInfo` at `index`
  match device_infos.read().await.get( index ) {
    Some( &device_info ) => {
      match etherdream::connect( device_info ).await {
        Ok( client ) => {
          // To simplify client management, we create a no-op
          // generator so we only have to persist generator-types.
          let generator = etherdream::make_generator( client, Box::new( executors::Noop::new() ) );
          generators.insert( index, generator );
          *current_index = Some( index );
          None
        }
        Err( err ) =>
          Some( format!( "({TEXT_CLIENT_ERROR}{:?})", err ) )
      }
    },
    None =>
      Some( "(unknown device)".to_owned() )
  }
}

async fn do_list( device_infos: &DeviceInfosRef ) -> Option<String> {
  let device_infos = device_infos.read().await;

  if device_infos.is_empty() {
    return Some( "(no devices)".to_owned() )
  }

  let copy: String =
    device_infos.iter()
      .enumerate()
      .map( |( index, device_info )| {
        format!( "  [{}] {} (MAC: {})\n", index, device_info.address().ip(), device_info.mac_address() )
      })
      .collect::<Vec<String>>()
      .join( "\n" );

  Some( copy )
}

async fn do_ping(
  device_infos: &DeviceInfosRef,
  generators: &mut GeneratorMap,
  current_index: Option<usize>
) -> Option<String> {
  let Some( index ) = current_index else {
    return Some( TEXT_NO_ACTIVE_DEVICE.to_string() )
  };

  let Some( generator ) = generators.get_mut( &index) else {
    return Some( TEXT_NOT_FOUND.to_owned() )
  };

  match generator.ping().await {
    Ok( state ) => {
      let guard = device_infos.read().await;
      let Some( device_info ) = guard.get( index ) else { return None };

      Some( format!( "Device:
  Buffer Capacity = {dac_buffer_capacity}
  Mac Address = {dac_mac_address}
  Max points per second = {dac_max_pps},
  Version = hardware: {dac_version_hw}; software: {dac_version_sw}
State:
  Light engine = {state_light_engine:?}
  Playback = {state_playback:?}
    E-stop = {state_playback_e_stop:?}
    Shutter = {state_playback_shutter:?}
    Underflow = {state_playback_underflow:?}
  Points = {state_points_buffered}
    Lifetime = {state_points_lifetime}
    Rate per second = {state_points_per_second}
  Source = {state_source:?}",
                     dac_buffer_capacity = device_info.buffer_capacity(),
                     dac_mac_address = device_info.mac_address(),
                     dac_max_pps = device_info.max_points_per_second(),
                     dac_version_hw = device_info.version().hardware,
                     dac_version_sw = device_info.version().software,
                     state_light_engine = state.light_engine_state(),
                     state_playback = state.playback_state(),
                     state_playback_e_stop = state.is_e_stop(),
                     state_playback_shutter = state.is_shutter_open(),
                     state_playback_underflow = state.is_underflow(),
                     state_points_buffered = state.points_buffered(),
                     state_points_lifetime = state.points_lifetime(),
                     state_points_per_second = state.points_per_second(),
                     state_source = state.source()
      ) )
    }
    Err( err ) =>
      Some( format!( "({TEXT_CLIENT_ERROR}{:?})", err ) )
  }
}

async fn do_play(
  generators: &mut GeneratorMap,
  current_index: Option<usize>,
  generator_name: &str
) -> Option<String> {
  let Some( index ) = current_index else {
    return Some( TEXT_NO_ACTIVE_DEVICE.to_owned() )
  };

  let Some( generator ) = generators.remove( &index ) else {
    return Some( TEXT_NOT_FOUND.to_owned() )
  };

  // Create an executor for the provided `generator_name`
  let executor: Box<dyn generator::Executable> =
    match generator_name.to_uppercase().as_str() {
      "DEMO" => Box::new( executors::Demo::new() ),
      _ => {
        generators.insert( index, generator );
        return Some( "(unknown generator)".to_owned() )
      }
    };

  // Acquire the underlying `Client` from the current `generator`
  let client =
    match generator.into_client().await {
      Ok( client ) => client,
      Err( err ) => {
        return Some( format!( "({TEXT_CLIENT_ERROR}{:?})", err ) )
      }
    };

  // Create and start a generator with our `client` and `executor`
  let mut generator = etherdream::make_generator( client, executor );
  generator.start().await;
  generators.insert( index, generator );
  None
}

async fn do_stop( generators: &mut GeneratorMap, current_index: Option<usize> ) -> Option<String> {
  let Some( index ) = current_index else {
    return Some( TEXT_NO_ACTIVE_DEVICE.to_owned() );
  };

  let Some( generator ) = generators.remove( &index ) else {
    return Some( TEXT_NOT_FOUND.to_owned() );
  };

  match generator.into_client().await {
    Ok( client ) => {
      let generator = etherdream::make_generator( client, Box::new( executors::Noop::new() ) );
      generators.insert( index, generator );
      None
    }
    Err( err ) =>
      Some( format!( "({TEXT_CLIENT_ERROR}{:?})", err ) )
  }
}