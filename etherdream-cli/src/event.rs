use std::time::Duration;

use crossterm::event::{ self, EventStream, KeyEvent, KeyEventKind };
use futures::{ FutureExt, StreamExt };
use tokio::sync::mpsc;

const FPS: f64 = 30.0;

pub enum Event {
  KeyEvent( KeyEvent ),
  Tick
}

pub struct EventHandler {
  tx: mpsc::Sender<Event>
}

impl EventHandler {
  pub fn new() -> ( Self, mpsc::Receiver<Event> ) {
    let ( tx, rx ) = mpsc::channel( 16 );
    ( Self{ tx }, rx )
  }

  pub async fn run( &self ) {
    let mut crossterm_events = EventStream::new();
    let tick_rate = Duration::from_secs_f64( 1.0 / FPS );
    let mut tick = tokio::time::interval( tick_rate );

    loop {
      tokio::select! {
        _ = self.tx.closed() => {
          break
        }
        _ = tick.tick() => {
          self.send( Event::Tick ).await
        }
        Some( Ok( crossterm_event ) ) = crossterm_events.next().fuse() => {
          match crossterm_event {
            event::Event::Key( key ) => {
              if key.kind == KeyEventKind::Press {
                self.send( Event::KeyEvent( key ) ).await;
              }
            }
            _ => {}
          }
        }
      }
    }
  }

  async fn send( &self, event: Event ) {
    let _ = self.tx.send( event ).await;
  }
}