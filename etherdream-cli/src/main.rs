//! CLI tool to discover, connect and test Etherdream DAC's.
use std::io::{ self, Write };

use clap;

use etherdream_cli::App;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Main

#[tokio::main( flavor="current_thread" )]
async fn main() {
  let mut app = App::new().await;
  let mut input = String::new();

  // Define the REPL interface using CLAP
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
        .about( "Will execute for `duration` seconds." )
        .arg( clap::Arg::new( "duration" ).required( true ).default_value( "5" ).value_parser( clap::value_parser!( u64 ) ) ),
      clap::Command::new( "reset" )
        .about( "Empties the point buffer in the DAC." ),
    ]);

  // Start the REPL
  loop {
    // Print console prefix
    if let Some( index ) = app.current_client_index() {
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
            Some(( "play", args )) => app.do_play_current_device( *args.get_one::<u64>( "duration" ).unwrap() ).await,
            Some(( "reset", _ )) => app.do_reset_current_device().await,
            _ => { /* unrecognized subcommand, no-op */ }
          },
        Err( _ ) => {}
      }
    }
  }
}