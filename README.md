# Etherdream

A library for ***discovering***, ***connecting*** and ***writing*** laser data 
to Etherdream devices. Built on [tokio](https://tokio.rs/).

> [!WARNING]
> This library is a work-in-progress. Use at your own risk.

This library has been developed and tested with the following hardware:
- Windows 11
- Etherdream 2
- Kvant Clubmax 2000 FB4 (30K pps@8deg)

## Discovery
The discovery server allows for finding Etherdream device's on a network. Each 
discovered device will be published exactly one time to the user-provided 
`mpsc::Sender`. The received `DiscoveredDeviceInfo` can be used to establish a 
connection with an Etherdream.

Example:
```rust,no_run
let ( tx, mut rx ) = tokio::sync::mpsc::channel( 16 );
let _ = etherdream::discover( tx ).await;

while let Some( device_info ) = rx.recv().await {
  // ...
}
```

## Client
A `Client` sends commands and receives responses from an Etherdream DAC. It 
provides for querying the Etherdream's status, sending point data, starting 
playback, and more.

A `Client` using the default configuration can be created using a `DeviceInfo` 
received from the discovery server. Example:
```rust,no_run
let client =
  match etherdream::connect( device_info ).await {
    Ok( client ) => client,
    Err( msg ) => panic!( "Failed to connect to Etherdream DAC: {:?}", msg )
  }
```

The `Client` behavior can be customized using `etherdream::client::Builder`. 
Reference `etherdream::client` for documentation on available configuration 
options and functions.

## Generator
A `Generator` consumes a `Client` and provides for pull-based point-data 
publishing as client buffer capacity becomes available. A user-provided 
`generator::Executable` will be invoked each time the `Client`'s internal point 
buffer drops below a configured threshold.

Each invocation of `Executable::execute` is provided an 
`generator::ExecutionContext` which allows for the querying and publishing of 
point data. User-defined data is accessible via `&mut self`.

```rust,no_run
use etherdream::generator;

pub struct MyExecutor { /* user-defined data */ }

impl generator::Executable for MyExecutor { 
  fn on_start( &mut self, ctx: generator::OnStartContext ) -> generator::Config {
    generator::Config::new( 1_000 )
  }
  
  fn execute( &mut self, ctx: &mut generator::ExecutionContext ) {
    while ctx.remaining() > 0 { 
      ctx.push_point( 0, 0, 0, 0, 0 );
    }
  }
}

let generator = etherdream::make_generator( client, Box::new( MyExecutor{} ) );
generator.start();
```

Reference `etherdream::generator` for documentation on available functions.