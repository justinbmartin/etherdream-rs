# Etherdream

A library for ***discovering***, ***connecting*** and ***writing*** laser data to Etherdream devices. Built on [tokio](https://tokio.rs/).

> [!WARNING]
> This library is a strong work-in-progress as I learn Rust. Use at your own risk.

## Discovery Server
This crate includes a discovery server for finding Etherdream DAC's on a 
network. Using `serve` will listen for UDP messages on the Etherdream-defined 
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
This crate includes a `Client` that can connect to and control an Etherdream 
DAC.

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