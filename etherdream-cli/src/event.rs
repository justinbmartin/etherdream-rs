use std::net::SocketAddr;
use std::time::Duration;

use crossterm::event::KeyEvent;
use futures::{ FutureExt, StreamExt };
use tokio::sync::mpsc;

const FPS: f64 = 30.0;

pub enum Event<'a> {
  AppEvent( AppEvent<'a> ),
  KeyEvent( KeyEvent ),
  Tick
}

pub enum AppEvent<'a> {
  Connect( &'a SocketAddr )
}

pub struct EventHandler<'a> {
  tx: mpsc::Sender<Event<'a>>
}

impl<'a> EventHandler<'a> {
  pub fn new() -> ( Self, mpsc::Receiver<Event<'a>> ) {
    let ( tx, rx ) = mpsc::channel( 16 );
    ( Self{ tx }, rx )
  }

  pub async fn run( &self ) {
    let tick_rate = Duration::from_secs_f64( 1.0 / FPS );
    let mut tick = tokio::time::interval( tick_rate );

    let mut reader = crossterm::event::EventStream::new();

    loop {
      let tick_delay = tick.tick();

      tokio::select! {
        _ = self.tx.closed() => break,
        _ = tick_delay => self.send( Event::Tick ).await,
        Some( Ok( evt ) ) = reader.next().fuse() => {
          if evt.is_key_press() {
            self.send( Event::KeyEvent( evt.as_key_event().unwrap() ) ).await;
          }
        }
      }
    }
  }

  async fn send( &self, event: Event<'a> ) {
    let _ = self.tx.send( event ).await;
  }
}