//! Generator: Utilities for pull-based point-data generation.
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::client::{ self, Client };
use crate::device_info::DeviceInfo;
use crate::protocol::{ X, Y, R, G, B };

const DEFAULT_LOW_WATERMARK_PERCENTAGE: f32 = 0.7;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Executable Trait

pub trait Executable: Send + Sync {
  /// Called upon `Generator::start`, before any call to `Executable::execute`.
  ///
  /// The `OnStartContext` provides information about the Etherdream device and
  /// the `Client`'s point buffer capacity. This information is useful in
  /// defining the generator's `Config`, the required return value.
  fn on_start( &mut self, ctx: OnStartContext ) -> Config;

  /// Called any time the underlying `Client`'s point buffer descends below
  /// the configured low-watermark threshold. This routine is the primary
  /// work-horse driving point data generation.
  ///
  /// Point data can be queried and published using the provided
  /// `ExecutionContext`.
  fn execute( &mut self, ctx: &mut ExecutionContext );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Config

#[derive( Debug )]
pub struct Config {
  low_watermark: f32,
  point_rate: usize
}

impl Config {
  /// Creates a new `Config` with the provided `point_rate` (points-per-second).
  pub fn new( point_rate: usize ) -> Self {
    Self {
      low_watermark: DEFAULT_LOW_WATERMARK_PERCENTAGE,
      point_rate
    }
  }

  /// Sets the percentage threshold at which the `Generator` will call the
  /// user-provided `Executable`.
  ///
  /// The `amount` will be clamped to a value between `[0.0, 1.0]`. The
  /// user-provided `Executable` will be called when the `Client`'s point count
  /// descends below this threshold.
  pub fn low_watermark( &mut self, amount: f32 ) {
    self.low_watermark = amount.clamp( 0.0, 1.0 );
  }

  /// Sets the default point rate that the generator will play point-data.
  pub fn point_rate( &mut self, rate: usize ) {
    self.point_rate = rate;
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Generator

#[derive( Debug )]
pub enum Error {
  Internal( String )
}

struct Task {
  handle: JoinHandle<( client::PointTx, Box<dyn Executable>, Option<client::Error> )>,
  shutdown_token: CancellationToken
}

/// A `Generator` allows for pull-based point-data publishing.
///
/// The generator is not started on instantiation. Call `Generator::start()` to
/// start the generator.
///
/// To use, an author must provide a `client::Client` and a struct that
/// implements `generator::Executable`. The executable provides a means for
/// authors to persist state between `Executable::execute` invocations. This
/// state is accessible via `&mut self`.
///
/// During an invocation, the `Executable` is also provided an
/// `generator::ExecutionContext`. This context allows for point-data querying
/// (via call's to `ctx.remaining()`, `ctx.max_points()`, etc.) and point-data
/// publishing (via `ctx.push_point(...)`).
pub struct Generator {
  client: client::ReadOnlyClient,
  command_tx: client::CommandTx,
  executor: Option<Box<dyn Executable>>,
  point_tx: Option<client::PointTx>,
  task: Option<Task>
}

impl Generator {
  /// Creates a `Generator` from a `Client` and `executor`. The generator is
  /// not started on instantiation.
  pub fn new( client: Client, executor: Box<dyn Executable> ) -> Self {
    let ( read_only_client, command_tx, point_tx ) = client.into_parts();

    Self{
      client: read_only_client,
      command_tx,
      executor: Some( executor ),
      point_tx: Some( point_tx ),
      task: None
    }
  }

  /// Returns true if the generator is running.
  pub fn is_running( &self ) -> bool {
    if let Some( task ) = self.task.as_ref() {
      ! task.handle.is_finished()
    } else {
      false
    }
  }

  /// Returns the `DeviceInfo` associated with the underlying `Client`.
  #[inline]
  pub fn device_info( &self ) -> &DeviceInfo {
    self.client.device_info()
  }

  /// Sends a ping to the connected device and awaits a response.
  #[inline]
  pub async fn ping( &mut self ) -> Result<client::State,client::Error> {
    self.command_tx.send_and_wait( client::Command::Ping ).await
  }
  
  /// Starts the generator, playing point data at the provided `rate` (points
  /// per second).
  pub async fn start( &mut self ) {
    if self.is_running() { return; }

    if let Some( mut executor ) = self.executor.take() && let Some( point_tx ) = self.point_tx.take() {
      let shutdown_token = CancellationToken::new();

      let handle = tokio::spawn({
        let mut command_tx = self.command_tx.clone().await;
        let device_info = self.client.device_info().clone();
        let shutdown_token = shutdown_token.child_token();
        let state = self.client.clone_state();

        async move {
          let config = executor.on_start( OnStartContext{ device_info: &device_info, capacity: point_tx.capacity() } );
          let low_watermark = ( point_tx.capacity() as f32 * config.low_watermark ) as usize;
          let mut ctx = ExecutionContext{ max_points: 0, point_count: 0, point_tx };

          loop {
            if shutdown_token.is_cancelled() {
              return ( ctx.point_tx, executor, None );
            }

            if state.read().await.is_ready() && ctx.point_tx.len() <= low_watermark {
              // Copy the point publisher's remaining capacity into
              // `ctx.max_points` to provide a constant, non-changing value for
              // the duration of the execution.
              ctx.max_points = ctx.point_tx.remaining();

              // Reset the context's published point count to zero. This is
              // incremented each time the executor publishes a point. The
              // executor will not be allowed to publish more points than
              // `ctx.max_points`.
              ctx.point_count = 0;

              // Run the executor with the configured execution context.
              executor.execute( &mut ctx );

              // Flush any published point data to the `Client::Writer` task.
              if ctx.point_count > 0 {
                match command_tx.send_and_wait( client::Command::Data{ count: ctx.point_count } ).await {
                  Ok( state ) => {
                    if ! state.is_playing() {
                      let _ = command_tx.send_and_wait( client::Command::Begin{ rate: config.point_rate } ).await;
                    }
                  }
                  Err( err ) => {
                    return ( ctx.point_tx, executor, Some( err ) );
                  }
                }
              }
            }

            // Yield control back to the tokio run-time
            tokio::time::sleep( Duration::from_millis( 1 ) ).await;
          }
        }
      });

      self.task = Some( Task{ handle, shutdown_token } );
    }
  }

  /// Stops the `Generator`.
  ///
  /// This action deconstructs the asynchronous task. The generator can be
  /// re-started by calling `Generator::start`.
  pub async fn stop( &mut self ) -> Result<(),Error> {
    if let Some( task ) = self.task.take() {
      let _ = self.command_tx.send_and_wait( client::Command::Stop ).await;
      let _ = self.command_tx.send_and_wait( client::Command::Prepare ).await;
      task.shutdown_token.cancel();

      match task.handle.await {
        Ok( ( point_tx, executor, err ) ) => {
          self.executor = Some( executor );
          self.point_tx = Some( point_tx );

          if let Some( err ) = err {
            Err( Error::Internal( format!( "Client Error: {err}" ) ) )
          } else {
            Ok( () )
          }
        },
        Err( err ) => Err( Error::Internal( format!( "Join Error: {}", err.to_string() ) ) )
      }
    } else {
      Ok( () )
    }
  }

  /// Stops the generator and returns the underlying `Client`, consuming `self`.
  pub async fn into_client( mut self ) -> Result<Client, Error> {
    self.stop().await?;

    if let Some( point_tx ) = self.point_tx.take() {
      Ok( Client::from_parts( self.client, self.command_tx, point_tx ) )
    } else {
      Err( Error::Internal( "Point publisher was not returned to generator".to_owned() ) )
    }
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - On Start Context

pub struct OnStartContext<'a> {
  device_info: &'a DeviceInfo,
  capacity: usize
}

impl<'a> OnStartContext<'a> {
  /// Returns the `DeviceInfo` associated with the underlying `Client`.
  pub fn device_info( &self ) -> &DeviceInfo { self.device_info }

  /// Returns the maximum capacity of the `Client`'s internal point buffer.
  pub fn point_capacity( &self ) -> usize { self.capacity }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Execution Context

pub struct ExecutionContext {
  // The maximum number of points that be published in this context.
  max_points: usize,
  // The incrementing number of points published during this context.
  point_count: usize,
  // The point data publisher.
  point_tx: client::PointTx
}

impl ExecutionContext {
  /// Returns the number of points published during this invocation.
  pub fn point_count( &self ) -> usize {
    self.point_count
  }

  /// Returns the remaining number of points that can be published during this
  /// invocation.
  pub fn remaining( &self ) -> usize {
    self.max_points.saturating_sub( self.point_count )
  }

  /// Returns the maximum number of points that can be published during this
  /// invocation. This is a constant value for the duration of an invocation.
  pub fn max_points( &self ) -> usize {
    self.max_points
  }

  /// Pushes a point into the point buffer if capacity is available.
  pub fn push_point( &mut self, x: X, y: Y, r: R, g: G, b: B ) -> Option<client::Point> {
    if self.point_count < self.max_points {
      let result = self.point_tx.push( ( x, y, r, g, b ) );
      if result.is_none() { self.point_count += 1; }
      result
    } else {
      Some( ( x, y, r, g, b ) )
    }
  }
}