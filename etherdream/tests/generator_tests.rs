use tokio::sync::mpsc;
use tokio::time::{ Duration, sleep, timeout };

use etherdream::client;
use etherdream::generator;
use etherdream_test::emulator::Emulator;

struct TestExecutor{
  invoked: mpsc::Sender<usize>,
  low_watermark: Option<f32>
}

impl generator::Executable for TestExecutor {
  fn on_start( &mut self, _: generator::OnStartContext ) -> generator::Config {
    let mut config = generator::Config::new( 1_000 );

    if let Some( low_watermark ) = self.low_watermark {
      config.low_watermark( low_watermark );
    }

    config
  }

  fn execute( &mut self, ctx: &mut generator::ExecutionContext ) {
    while ctx.remaining() > 0 {
      ctx.push_point( 1, 2, 3, 4, 5 );
    }

    let _ = self.invoked.try_send( ctx.point_count() );
  }
}

#[tokio::test]
async fn a_generator_will_publish_point_data() {
  let mut emulator = Emulator::start_with_capacity( 10 ).await.unwrap();

  let client = client::Builder::new( emulator.get_device_info() )
    .capacity( 10 )
    .connect().await
    .unwrap();

  let ( invoked_tx, mut invoked_rx ) = mpsc::channel::<usize>( 16 );
  let executor = Box::new( TestExecutor{ invoked: invoked_tx, low_watermark: None } );
  let mut generator = generator::Generator::new( client, executor );
  generator.start().await;

  // The executor will be invoked twice to saturate both the device and client
  // buffers (20 points, together).
  assert_eq!( wait_for_executor_invocation( &mut invoked_rx ).await, 10 );
  assert_eq!( wait_for_executor_invocation( &mut invoked_rx ).await, 10 );

  // Validate that the emulator is at capacity
  assert!( wait_for_emulator_count( &emulator, 10 ).await );

  // Consume 1 point from the emulator.
  //
  // The executor will not be invoked since the client's point count has not
  // descended below the default 70% threshold. After `consume_points( 1 )`,
  // the client will have 9 points in its buffer, the emulator device will
  // still have 10 points.
  emulator.consume_points( 1 );
  assert!( wait_for_emulator_count( &emulator, 10 ).await );

  // Consume 3 points from the emulator.
  //
  // This will invoke the executor since the client's point count will descend
  // below the default 70% threshold. After `consume_points( 3 )`, the client
  // will have 6 points in its buffer.
  emulator.consume_points( 3 );
  assert_eq!( wait_for_executor_invocation( &mut invoked_rx ).await, 4 );
  assert!( wait_for_emulator_count( &emulator, 10 ).await );

  // Stop the generator
  let client = generator.into_client().await.unwrap();
  client.disconnect().await;
}

#[tokio::test]
async fn a_generator_can_be_configured_with_a_custom_low_watermark() {
  let mut emulator = Emulator::start_with_capacity( 10 ).await.unwrap();

  let client = client::Builder::new( emulator.get_device_info() )
    .capacity( 10 )
    .connect().await
    .unwrap();

  let ( invoked_tx, mut invoked_rx ) = mpsc::channel::<usize>( 16 );
  let executor = Box::new( TestExecutor{ invoked: invoked_tx, low_watermark: Some( 0.4 ) } );

  let mut generator = generator::Generator::new( client, executor );
  generator.start().await;

  // The executor will be invoked twice
  assert_eq!( wait_for_executor_invocation( &mut invoked_rx ).await, 10 );
  assert_eq!( wait_for_executor_invocation( &mut invoked_rx ).await, 10 );

  // Validate that the emulator is at capacity
  assert!( wait_for_emulator_count( &emulator, 10 ).await );

  // Consume 5 points from the emulator. The low watermark threshold (at 4
  // points) will not be triggered.
  emulator.consume_points( 5 );
  assert!( wait_for_emulator_count( &emulator, 10 ).await );

  // Consume 1 point from the emulator. This will trigger the low watermark
  // threshold (at 4 points) and generate 6 points to saturate the client's
  // point buffer.
  emulator.consume_points( 1 );
  assert_eq!( wait_for_executor_invocation( &mut invoked_rx ).await, 6 );
}

#[tokio::test]
async fn a_generator_can_ping_the_device() {
  let emulator = Emulator::start_with_capacity( 10 ).await.unwrap();

  let client = client::Builder::new( emulator.get_device_info() )
    .capacity( 10 )
    .connect().await
    .unwrap();

  let ( invoked_tx, _invoked_rx ) = mpsc::channel::<usize>( 16 );
  let executor = Box::new( TestExecutor{ invoked: invoked_tx, low_watermark: None } );
  let mut generator = generator::Generator::new( client, executor );
  generator.start().await;

  let state = generator.ping().await.unwrap();
  assert_eq!( state.points_buffered(), 0 );
}

// Waits for emulator to report `count` points, or terminates in 1sec.
async fn wait_for_emulator_count( emulator: &Emulator, count: usize ) -> bool {
  timeout( Duration::from_secs( 1 ), async move {
    loop {
      if emulator.point_count() == count {
        return ();
      }

      sleep( Duration::from_millis( 1 ) ).await
    }
  }).await.is_ok()
}

async fn wait_for_executor_invocation( invoked_rx: &mut mpsc::Receiver<usize> ) -> usize {
  match timeout( Duration::from_secs( 1 ), invoked_rx.recv() ).await {
    Ok( Some( count ) ) => count,
    Ok( None ) => 0,
    Err( _ ) => 0
  }
}