use etherdream::circular_buffer::{ CircularBuffer, PushError };

#[test]
fn buffer_push_and_pop_test() {
  let ( mut writer, mut reader ) = CircularBuffer::<&str,2>::new();

  // The buffer is empty
  assert_eq!( reader.capacity(), 2 );
  assert_eq!( reader.is_empty(), true );
  assert_eq!( reader.is_full(), false );
  assert_eq!( reader.len(), 0 );
  assert_eq!( reader.remaining(), 2 );

  assert_eq!( writer.capacity(), 2 );
  assert_eq!( writer.is_empty(), true );
  assert_eq!( writer.is_full(), false );
  assert_eq!( writer.len(), 0 );
  assert_eq!( writer.remaining(), 2 );

  // The buffer has a single item
  let _ = writer.push( "a" );

  assert_eq!( reader.is_empty(), false );
  assert_eq!( reader.is_full(), false );
  assert_eq!( reader.len(), 1 );
  assert_eq!( reader.remaining(), 1 );

  assert_eq!( writer.is_empty(), false );
  assert_eq!( writer.is_full(), false );
  assert_eq!( writer.len(), 1 );
  assert_eq!( writer.remaining(), 1 );

  // The buffer is at capacity
  let _ = writer.push( "b" );

  assert_eq!( reader.is_empty(), false );
  assert_eq!( reader.is_full(), true );
  assert_eq!( reader.len(), 2 );
  assert_eq!( reader.remaining(), 0 );

  assert_eq!( writer.is_empty(), false );
  assert_eq!( writer.is_full(), true );
  assert_eq!( writer.len(), 2 );
  assert_eq!( writer.remaining(), 0 );
  
  // Will return an insufficient capacity error if we attempt to push again
  assert_eq!( writer.push( "c" ).err(), Some( PushError::InsufficientCapacity ) );

  // The caller has consumed a single item
  assert_eq!( "a", reader.pop().unwrap() );

  assert_eq!( reader.is_empty(), false );
  assert_eq!( reader.is_full(), false );
  assert_eq!( reader.len(), 1 );

  assert_eq!( writer.is_empty(), false );
  assert_eq!( writer.is_full(), false );
  assert_eq!( writer.len(), 1 );

  // The caller has consumed all items
  assert_eq!( "b", reader.pop().unwrap() );

  assert_eq!( reader.is_empty(), true );
  assert_eq!( reader.is_full(), false );
  assert_eq!( reader.len(), 0 );

  assert_eq!( writer.is_empty(), true );
  assert_eq!( writer.is_full(), false );
  assert_eq!( writer.len(), 0 );

  // The buffer will return None since all items have been consumed
  assert!( reader.pop().is_none() );
}

#[test]
fn buffer_will_handle_rollover_test() {
  let ( mut writer, mut reader ) = CircularBuffer::<&str,3>::new();

  let _ = writer.push( "a" );
  let _ = writer.push( "b" );
  let _ = reader.pop();
  let _ = writer.push( "c" );

  assert_eq!( writer.is_empty(), false );
  assert_eq!( writer.is_full(), false );
  assert_eq!( writer.len(), 2 );

  let _ = writer.push( "d" );
  assert_eq!( writer.is_empty(), false );
  assert_eq!( writer.is_full(), true );
  assert_eq!( writer.len(), 3 );
}

/*
#[test]
fn buffer_push_from_slice_test() {
  let mut buffer = BoundedCircularBuffer::<&str,10>::new();

  let _ = buffer.push_from_slice([ "a, "b", "c" ]);

  assert_eq!( buffer.is_empty(), false );
  assert_eq!( buffer.is_full(), false );
  assert_eq!( buffer.len(), 5 );
  assert_eq!( buffer.capacity(), 5 );

  let _ = buffer.push_from_slice( &points );

  assert_eq!( buffer.is_empty(), false );
  assert_eq!( buffer.is_full(), true );
  assert_eq!( buffer.len(), 10 );
  assert_eq!( buffer.capacity(), 0 );
}
*/