use std::net::SocketAddr;

use tokio::io::{ self, AsyncReadExt, AsyncWriteExt };
use tokio::net;
use tokio::sync::mpsc;
use tokio::task;
use tokio_util::sync::CancellationToken;

//use tokio::net::TcpStream;
//use tokio_util::codec::{ BytesCodec, FramedRead };

use crate::device::State;

pub const DEFAULT_PORT: u16 = 7765;

#[repr( C )]
struct EtherdreamResponse {
  // The control signal of the response, can be:
  // 'a':	ACK (0x61) - Acknowledged. The previous command was accepted.
  // 'F': NAK (0x46) - Full. The write command could not be performed because 
  //      there was not enough buffer space when it was received.
  // 'I': NAK (0x49) - Invalid. The command contained an invalid command byte 
  //      or parameters.
  // '!': NAK (0x21) - Stop Condition. An emergency-stop condition still 
  //      exists.
  signal: u8,

  // An echo of the command sent.
  command: u8,
  state: State
}

enum Command {
  Ping
}

pub struct Client {
  address: SocketAddr,

  //
  cancellable: CancellationToken,

  //
  command_tx: mpsc::UnboundedSender<Command>,
  
  //
  rx_handle: task::JoinHandle<io::Result<()>>,
  tx_handle: task::JoinHandle<io::Result<()>>
}

impl Client {
  pub fn remote( &self ) -> SocketAddr {
    return self.address;
  }

  pub fn ping( &mut self ) {
    // push callback?
    let _ = self.send( Command::Ping );
  }

  fn send( &mut self, command: Command ) -> Result<(),mpsc::error::SendError<Command>> {
    return self.command_tx.send( command );
  }

  // consumes self?
  pub async fn stop( self ) {
    if ! self.cancellable.is_cancelled() {
      self.cancellable.cancel();
    }

    let _ = self.rx_handle.await;
    let _ = self.tx_handle.await;
  }
}

pub struct Builder {
  duration: Option<tokio::time::Duration>,
  remote_address: SocketAddr
}

impl Builder {
  pub fn new( remote_address: SocketAddr ) -> Self {
    return Self{ 
      duration: None,
      remote_address: remote_address
    };
  }

  pub fn duration( &mut self, duration: tokio::time::Duration ) -> &mut Self {
    self.duration = Some( duration );
    return self;
  }

  pub async fn start( &self ) -> io::Result<Client> {
    let ( command_tx, command_rx ) = mpsc::unbounded_channel::<Command>();

    let cancellable = CancellationToken::new();

    let socket = net::TcpSocket::new_v4()?;
    let stream = socket.connect( self.remote_address ).await?;
    let ( rx, tx ) = stream.into_split();

    // Spawn read handler
    let rx_handle = {
      let cancellable = cancellable.clone();

      tokio::spawn( async move {
        tokio::select!{
          _ = cancellable.cancelled() => { Ok(()) }
          result = do_read( rx ) => { result }
        }
      })
    };

    // Spawn write handler
    let tx_handle = {
      let cancellable = cancellable.clone();

      tokio::spawn( async move {
        tokio::select!{
          _ = cancellable.cancelled() => { Ok(()) }
          result = do_write( command_rx, tx ) => { result }
        }
      })
    };

    if let Some( duration ) = self.duration {
      tokio::spawn({
        let cancellable = cancellable.clone();

        async move {
          tokio::time::sleep( duration ).await;
          cancellable.cancel();
        }
      });
    }

    return Ok( Client{ 
      address: self.remote_address,
      command_tx: command_tx,
      cancellable: cancellable,
      rx_handle: rx_handle,
      tx_handle: tx_handle
    });
  }
}

async fn do_read( mut rx: net::tcp::OwnedReadHalf ) -> io::Result<()> {
  let mut buf = [0 as u8; 2];

  loop {
    let _ = rx.read_exact( &mut buf ).await?;
    
    if buf == *b"ap" {
      println!( "> CLIENT: ACK!" );
    } else {
      println!( "> CLIENT: unknown reply" );
    }
  }
}

async fn do_write( mut command_rx: mpsc::UnboundedReceiver<Command>, mut tx: net::tcp::OwnedWriteHalf ) -> io::Result<()> {
  loop {
    while let Some( command ) = command_rx.recv().await {
      match command {
        Command::Ping => {
          let _ = tx.write( b"p" ).await?;
        }
      }
    }
  }
}
