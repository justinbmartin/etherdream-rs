use etherdream::point::{ Point, PointBuffer };

#[test]
fn buffer_push_and_pop_test() {
  let mut buffer = PointBuffer::<2>::new();
  let point = Point::new( 1, 1, u16::MAX, 0, 0 );

  // The buffer is empty
  assert_eq!( buffer.count(), 0 );
  assert_eq!( buffer.is_empty(), true );
  assert_eq!( buffer.is_full(), false );
  assert_eq!( buffer.capacity(), 2 );

  // The buffer has a single point
  buffer.push( point );

  assert_eq!( buffer.is_empty(), false );
  assert_eq!( buffer.is_full(), false );
  assert_eq!( buffer.count(), 1 );
  assert_eq!( buffer.capacity(), 1 );

  // Thue buffer is at capacity
  buffer.push( point );

  assert_eq!( buffer.is_empty(), false );
  assert_eq!( buffer.is_full(), true );
  assert_eq!( buffer.count(), 2 );
  assert_eq!( buffer.capacity(), 0 );

  // The caller has consumed a single point
  let _point = buffer.pop().unwrap();

  assert_eq!( buffer.is_empty(), false );
  assert_eq!( buffer.is_full(), false );
  assert_eq!( buffer.count(), 1 );
  assert_eq!( buffer.capacity(), 1 );

  // The caller has consumed all points
  let _point = buffer.pop().unwrap();

  assert_eq!( buffer.is_empty(), true );
  assert_eq!( buffer.is_full(), false );
  assert_eq!( buffer.count(), 0 );
  assert_eq!( buffer.capacity(), 2 );

  // The buffer will return None since all points have been consumed
  assert!( buffer.pop().is_none() );
}