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
    let fps = Duration::from_secs_f64( 1.0 / FPS );
    let mut interval = tokio::time::interval( fps );

    loop {
      tokio::select! {
        _ = self.tx.closed() => {
          break
        }
        _ = interval.tick() => {
          let _ = self.tx.send( Event::Tick ).await;
        }
        Some( Ok( crossterm_event ) ) = crossterm_events.next().fuse() => {
          match crossterm_event {
            event::Event::Key( key ) => {
              if key.kind == KeyEventKind::Press {
                let _ = self.tx.send( Event::KeyEvent( key ) ).await;
              }
            }
            _ => { /* no-op */ }
          }
        }
      }
    }
  }
}