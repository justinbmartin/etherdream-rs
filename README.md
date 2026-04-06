# Etherdream

A library for ***discovering***, ***connecting*** and ***writing*** laser data 
to Etherdream devices. Built on [tokio](https://tokio.rs/).

The Etherdream protocol: https://ether-dream.com/protocol.html

> [!WARNING]
> This library is a strong work-in-progress as I learn Rust. Use at your own risk.

## Discovery Server
The discovery server allows for finding Etherdream DAC's on a network. Use 
`Discovery::serve` to listen for UDP messages on the Etherdream-defined 
broadcast port, `7654`.

Example:
```rust
let ( tx, mut rx ) = tokio::sync::mpsc::channel( 16 );
let _ = etherdream::Discovery::serve( tx ).await;

while let Some( discovered_device_info ) = rx.recv().await {
  // Handle the `discovered_device_info`
}
```

## Client
The `Client` allows for connecting to and controlling an Etherdream DAC.

A `DeviceInfo` received from the discovery server can be used to create a 
`Client`:
```rust
let client =
  match etherdream::connect( device_info ).await {
    Ok( client ) => client,
    Err( msg ) => panic!( "Failed to connect to Etherdream DAC: {:?}", msg )
  }
```

Reference `etherdream::client` for documentation on available functions.

## Generator
The `Generator` consumes a `Client` and a user-provided `Executable` to allow
for active point data publishing.

The user-provided executable is any struct that implements
`generator::Executable`. The struct can hold any user-defined state that can
be accessed during each `execute` invocation via `&mut self`.

```rust
use etherdream::generator::{ Executable, ExecutionContext };

pub struct MyGenerator { /* user-defined state */ }

impl Executable for MyGenerator { 
  fn execute( &mut self, ctx: &mut ExecutionContext ) {
    while ctx.remaining() > 0 { 
      ctx.push_point( 0, 0, 0, 0, 0 );
    }
  }
}

let generator = etherdream::generator( client, Box::new( MyGenerator{} ) ).await;
```

Reference `etherdream::generator` for documentation on available functions.