use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::client::{ self, Client };
use crate::device_info;
use crate::protocol::{ X, Y, R, G, B };

const DEFAULT_LOW_WATERMARK_PERCENTAGE: f32 = 0.7;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Executable Trait

pub trait Executable: Send + Sync {
  fn execute( &mut self, ctx: &mut ExecutionContext );
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Generator Error

#[derive( Debug )]
pub enum Error {
  Internal( String )
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Config

pub struct Config {
  low_watermark: Option<usize>
}

impl Config {
  pub fn new() -> Self {
    Self{ low_watermark: None }
  }

  /// Defines the point threshold in which the `Generator` will call the
  /// `executor`. Value will be limited to the client's max buffer capacity.
  pub fn low_watermark( mut self, low_watermark: usize ) -> Self {
    self.low_watermark = Some( low_watermark );
    self
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Task

struct Task {
  handle: JoinHandle<( client::PointTx, Box<dyn Executable> )>,
  shutdown_token: CancellationToken
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Generator

pub struct Generator {
  client: client::ReadOnlyClient,
  command_tx: client::CommandTx,
  executor: Option<Box<dyn Executable>>,
  low_watermark: usize,
  point_tx: Option<client::PointTx>,
  task: Option<Task>
}

impl Generator {
  /// Create a `Generator`.
  pub fn new( client: Client, executor: Box<dyn Executable> ) -> Self {
    Self::with_config( client, executor, Config::new() )
  }

  /// Create a `Generator` with provided configuration.
  pub fn with_config( client: Client, executor: Box<dyn Executable>, config: Config ) -> Self {
    let ( read_only_client, command_tx, point_tx ) = client.into_parts();

    let low_watermark =
      if let Some( low_watermark ) = config.low_watermark {
        low_watermark.min( point_tx.capacity() )
      } else {
        ( point_tx.capacity() as f32 * DEFAULT_LOW_WATERMARK_PERCENTAGE ) as usize
      };

    Self{
      client: read_only_client,
      command_tx,
      executor: Some( executor ),
      low_watermark,
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

  /// Returns the `DeviceInfo` associated with the underlying client.
  #[inline]
  pub fn device_info( &self ) -> &device_info::DeviceInfo {
    self.client.device_info()
  }

  /// Sends a ping to the connected device and awaits a response.
  #[inline]
  pub async fn ping( &mut self ) -> Result<client::State,client::Error> {
    self.command_tx.send_and_wait( client::Command::Ping ).await
  }
  
  /// Start the generator.
  pub async fn start( &mut self ) {
    if self.is_running() { return; }

    if let Some( mut executor ) = self.executor.take() && let Some( point_tx ) = self.point_tx.take() {
      let shutdown_token = CancellationToken::new();

      let handle = tokio::spawn({
        let mut command_tx = self.command_tx.clone().await;
        let low_watermark = self.low_watermark;
        let shutdown_token = shutdown_token.child_token();
        let state = self.client.clone_state();

        async move {
          let mut ctx = ExecutionContext{ max_points: 0, point_count: 0, point_tx };

          loop {
            if shutdown_token.is_cancelled() {
              return ( ctx.point_tx, executor );
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

              // Run the executor with the configured context.
              executor.execute( &mut ctx );

              // Flush any published point data to the `Client::Writer` task.
              // TODO: Generator should provide error reporting, reset, etc.
              if ctx.point_count > 0 {
                match command_tx.send_and_wait( client::Command::Data{ count: ctx.point_count } ).await {
                  Ok( state ) => {
                    if ! state.is_playing() {
                      let _ = command_tx.send_and_wait( client::Command::Begin{ rate: 1000 } ).await;
                    }
                  }
                  Err( err ) => {
                    dbg!( err );
                    return ( ctx.point_tx, executor );
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
  pub async fn stop( &mut self ) -> Result<(),Error> {
    if let Some( task ) = self.task.take() {
      let _ = self.command_tx.send_and_wait( client::Command::Stop ).await;
      let _ = self.command_tx.send_and_wait( client::Command::Prepare ).await;
      task.shutdown_token.cancel();

      match task.handle.await {
        Ok( ( point_tx, executor ) ) => {
          self.executor = Some( executor );
          self.point_tx = Some( point_tx );
          Ok( () )
        },
        Err( err ) => Err( Error::Internal( err.to_string() ) )
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
      Err( Error::Internal( "".to_string() ) )
    }
  }
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
  /// Returns the number of points published during this context.
  pub fn point_count( &self ) -> usize {
    self.point_count
  }

  /// Returns the remaining number of points that can be published during this
  /// context.
  pub fn remaining( &self ) -> usize {
    self.max_points.saturating_sub( self.point_count )
  }

  /// Returns the maximum number of points that can be published during this
  /// context. This is a constant for the duration of a single executor
  /// invocation.
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