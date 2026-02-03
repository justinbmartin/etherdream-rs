# Etherdream

A library for ***discovering***, ***connecting*** and ***writing*** laser data 
to Etherdream devices. Built on [tokio](https://tokio.rs/).

The Etherdream protocol is defined here: https://ether-dream.com/protocol.html

> [!WARNING]
> This library is a strong work-in-progress as I learn Rust. Use at your own risk.

## TODO:
- [ ] Stop (and Reset?) client if NAK is received. This will prevent potential 
      lockup in `Client::Writer`.
- [ ] Add a default point implementation for client.
- [ ] Consider "on_error" callback. Let CLI development inform this.

## Discovery Server
The discovery server allows for finding Etherdream DAC's on a network. Use 
`Discovery::serve` to listen for UDP messages on the Etherdream-defined 
broadcast port, `7654`.

Example:
```rust
let ( tx, mut rx ) = tokio::sync::mpsc::channel( 16 );
tokio::spawn( etherdream::Discovery::serve( tx ) );

while let Some( device ) = rx.recv().await {
  dbg!( device );
}
```

## Client
The `Client` allows for connecting to and controlling an Etherdream DAC.

A `device` received from the discovery server can be used to create a `Client` 
like this:
```rust
let client =
  match etherdream::ClientBuilder::new( device ).connect().await {
    Ok( client ) => client,
    Err( msg ) => panic!( "Failed to connect to Etherdream DAC: {:?}", msg )
  }
```

Reference `etherdream::Client` for documentation on available functions.