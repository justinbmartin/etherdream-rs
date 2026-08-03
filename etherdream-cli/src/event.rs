use std::time::Duration;

use crossterm::event::{ self, EventStream, KeyEvent, KeyEventKind };
use futures::{ FutureExt, StreamExt };
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time;
use tokio_util::sync::CancellationToken;

const FPS: f32 = 30.0;

pub enum Event {
  KeyEvent( KeyEvent ),
  Tick( f64 )
}

pub struct EventController {
  cancellation_token: CancellationToken,
  tasks: JoinSet<()>
}

impl EventController {
  pub async fn start() -> ( EventController, mpsc::Receiver<Event> ) {
    let cancellation_token = CancellationToken::new();
    let mut tasks = JoinSet::new();
    let ( tx, rx ) = mpsc::channel( 16 );

    tasks.spawn({
      let cancellation_token = cancellation_token.child_token();
      let tx = tx.clone();

      // Start interval tick for FPS
      async move {
        tokio::select!{
          _ = cancellation_token.cancelled() => {}
          _ = async move {
            let fps = Duration::from_secs_f32( 1.0 / FPS );
            let mut interval = time::interval( fps );
            let start_time = time::Instant::now();

            loop {
              let now = interval.tick().await;
              let _ = tx.send( Event::Tick( ( start_time - now ).as_secs_f64() ) ).await;
            }
          } => {}
        }
      }
    });

    // Start task to capture key events
    tasks.spawn({
      let cancellation_token = cancellation_token.child_token();
      let tx = tx.clone();

      async move {
        tokio::select!{
          _ = cancellation_token.cancelled() => {}
          _ = async move {
            let mut crossterm_events = EventStream::new();

            loop {
              if let Some( Ok( crossterm_event ) ) = crossterm_events.next().fuse().await {
                match crossterm_event {
                  event::Event::Key( key ) => {
                    if key.kind == KeyEventKind::Press {
                      let _ = tx.send( Event::KeyEvent( key ) ).await;
                    }
                  }
                  _ => { /* no-op */ }
                }
              }
            }
          } => {}
        }
      }
    });

    let controller = EventController{ cancellation_token, tasks };
    ( controller, rx )
  }

  pub async fn stop( self ) {
    self.cancellation_token.cancel();
    self.tasks.join_all().await;
  }
}