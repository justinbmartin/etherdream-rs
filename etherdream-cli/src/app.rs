/// The primary application CLI logic, used by `main`.
use std::collections::HashMap;
use std::sync::Arc;

use etherdream;
use nalgebra as na;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::generators::default;
use crate::page::Page;

struct Client {
  client: etherdream::Client,
  _handle: JoinHandle<()>
}

pub struct App {
  // The REPL's active Etherdream client, by index in `clients`
  current_client: Option<usize>,

  // A map of actively connected Etherdream clients, indexed by a constant id
  clients: HashMap<usize,Client>,

  // A list of all discovered Etherdream devices
  devices: Arc<RwLock<Vec<etherdream::Device>>>,

  //
  _discovery_task: JoinHandle<()>
}

impl App {
  pub async fn new() -> Self {
    let devices = Arc::new( RwLock::new( Vec::new() ) );

    // Start the Etherdream discovery service. Each discovered device:
    // - Will be received async via `rx`.
    // - Will be persisted into `devices`.
    let discovery_task = tokio::spawn({
      let devices = devices.clone();

      async move {
        let ( tx, mut rx ) = tokio::sync::mpsc::channel( 16 );
        let _ = etherdream::Discovery::serve( tx ).await;

        while let Some( device ) = rx.recv().await {
          devices.write().await.push( device );
        }
      }
    });

    App{
      clients: HashMap::new(),
      current_client: None,
      devices,
      _discovery_task: discovery_task
    }
  }

  pub fn current_client_index( &self ) -> Option<usize> {
    self.current_client
  }

  pub async fn do_list_devices( &self ) {
    let devices = self.devices.read().await;

    if devices.is_empty() {
      println!( "(no devices)" );
    } else {
      for ( index, device ) in devices.iter().enumerate() {
        println!( "  [{}] {} (MAC: {})", index, device.address().ip(), device.mac_address() );
      }
    }
  }

  pub async fn do_connect( &mut self, device_index: usize ) {
    // If client already exists, set it as the current device. Else, create a
    // new client.
    if self.clients.contains_key( &device_index ) {
      self.current_client = Some( device_index );
    } else {
      let devices = self.devices.read().await;

      match devices.get( device_index ) {
        Some( &device ) => {
          let builder = etherdream::ClientBuilder::new( device )
            .on_low_watermark( 1000, | _count |{} );

          match builder.connect().await {
            Ok( client ) => {
              let handle = tokio::spawn( async {
                loop {

                }
              });
              let client = Client{ client, _handle: handle };

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

  pub async fn do_disconnect( &mut self ) {
    if let Some( index ) = self.current_client {
      if let Some( client ) = self.clients.remove( &index ) {
        let _ = client.client.disconnect().await;
      }
    }
  }

  pub async fn do_print_device_info( &self, device_index: usize ) {
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

  pub async fn do_ping_current_device( &mut self ) {
    if let Some( client_index ) = self.current_client {
      let client = self.clients.get_mut( &client_index ).unwrap();

      match client.client.ping().await {
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

  pub async fn do_play_current_device( &mut self, _duration_secs: u64 ) {
    if let Some( client_index ) = self.current_client {
      let client = self.clients.get_mut( &client_index ).unwrap();

      let mut buf = vec![na::Point3::<f64>::default(); 20_000];
      let page = Page::default();

      let _next = default::generate( &mut buf, page, 1_000, 0.25, 0.25 );

      for point in buf {
        client.client.push_point(
          lerp_f64_i16( -1.0, 1.0, point.x ),
          lerp_f64_i16( -1.0, 1.0, point.y ),
          u16::MAX,
          0,
          0
        );
      }

      // Start processing point data
      println!( "starting..." );
      let _ = client.client.start( 1000 ).await;

    } else {
      println!( "(no device connected)" );
    }
  }

  pub async fn do_reset_current_device( &mut self ) {
    if let Some( client_index ) = self.current_client {
      let client = self.clients.get_mut( &client_index ).unwrap();
      let _ = client.client.reset().await;
    } else {
      println!( "(no device connected)" );
    }
  }
}

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

#[inline]
fn lerp_f64_i16( min: f64, max: f64, amount: f64) -> i16 {
  ( na::clamp( amount, min, max ) * i16::MAX as f64 ).round() as i16
}