//! A bounded single-producer single-consumer (SPSC) circular buffer.
use std::mem::ManuallyDrop;
use std::sync::{ Arc, atomic::{ AtomicUsize, Ordering } };

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Circular Buffer

pub struct CircularBuffer<T, const CAPACITY: usize> {
  data: *mut T,
  size: AtomicUsize
}

impl<T, const CAPACITY: usize> CircularBuffer<T, CAPACITY>
where T: Copy + Clone + Default + Send
{
  /// Creates a new single-producer single-consumer (SPSC) bounded circular buffer.
  pub fn new() -> ( Writer<T, CAPACITY>, Reader<T, CAPACITY> )
  {
    let buffer = Arc::new( CircularBuffer::<T, CAPACITY>{
      data: ManuallyDrop::new( Box::new( [T::default(); CAPACITY] ) ).as_mut_ptr(),
      size: AtomicUsize::new( 0 )
    });

    (
      Writer::<T, CAPACITY> { buffer: buffer.clone(), head: 0 },
      Reader::<T, CAPACITY> { buffer: buffer.clone(), tail: 0 }
    )
  }
}

impl<T, const CAPACITY: usize> Drop for CircularBuffer<T, CAPACITY> {
  fn drop( &mut self ) {
    // SAFETY: Compliment to the `ManuallyDrop` that wraps `data`.
    unsafe { drop( Box::from_raw( self.data ) ) };
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Reader

pub struct Reader<T, const CAPACITY: usize>
{
  buffer: Arc<CircularBuffer<T, CAPACITY>>,
  tail: usize
}

impl<T, const CAPACITY: usize> Reader<T, CAPACITY>
{
  /// Returns true if the buffer is empty
  pub fn is_empty( &self ) -> bool {
    self.len() == 0
  }

  /// Returns true if the buffer is at capacity
  pub fn is_full( &self ) -> bool { 
    self.len() == CAPACITY
  }

  /// Returns the count of unread items in the buffer
  pub fn len( &self ) -> usize {
    self.buffer.size.load( Ordering::Relaxed )
  }

  /// Returns the count of remaining capacity in the buffer
  pub fn remaining( &self ) -> usize {
    CAPACITY - self.len()
  }

  /// Returns the maximum capacity of the buffer
  pub fn capacity( &self ) -> usize {
    CAPACITY
  }

  /// Returns the currently available item (if one is available) and advances the head. If the
  /// buffer is empty, returns None.
  pub fn pop( &mut self ) -> Option<T> {
    if self.is_empty() {
      return None;
    }

    let item = unsafe{ self.buffer.data.add( self.tail ).read() };
    self.tail = ( self.tail + 1 ) % CAPACITY;
    self.buffer.size.fetch_sub( 1, Ordering::Relaxed );
    Some( item )
  }
}

// SAFETY:
// * Compile-time guarantee that there is only one writer to the shared buffer.
// * Logical guarantee that the writer will never access the reader's tail.
unsafe impl<T: Send, const CAPACITY: usize> Send for Reader<T,CAPACITY> {}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Writer

pub struct Writer<T, const CAPACITY: usize>
{
  buffer: Arc<CircularBuffer<T, CAPACITY>>,
  head: usize
}

impl<T, const CAPACITY: usize> Writer<T, CAPACITY>
{
  /// Returns true if the buffer is empty
  pub fn is_empty( &self ) -> bool {
    self.len() == 0
  }

  /// Returns true if the buffer is at capacity
  pub fn is_full( &self ) -> bool { 
    self.len() == CAPACITY
  }

  /// Returns the count of unread items in the buffer
  pub fn len( &self ) -> usize {
    self.buffer.size.load( Ordering::Relaxed )
  }

  /// Returns the count of remaining capacity in the buffer
  pub fn remaining( &self ) -> usize {
    CAPACITY - self.len()
  }

  /// Returns the maximum capacity of the buffer
  pub fn capacity( &self ) -> usize {
    CAPACITY
  }

  /// Pushes `item` into the buffer (if space is available) and advances the tail. If the buffer is
  /// full, returns a PushError.
  pub fn push( &mut self, item: T ) -> Result<(),PushError> {
    if self.is_full() {
      return Err( PushError::InsufficientCapacity );
    }

    unsafe{ self.buffer.data.add( self.head ).write( item ); }
    self.head = ( self.head + 1 ) % CAPACITY;
    let _size = self.buffer.size.fetch_add( 1, Ordering::Relaxed );
    Ok(())
  }
}

// SAFETY:
// * Compile-time guarantee that there is only one reader to the shared buffer.
// * Logical guarantee that the reader will never consume from the writer's head.
unsafe impl<T: Send, const CAPACITY: usize> Send for Writer<T,CAPACITY> {}

#[derive( Debug, PartialEq )]
pub enum PushError {
  InsufficientCapacity
}

/*
  //
  pub fn push_from_slice( &mut self, points: &[T] ) -> Result<(),BoundedCircularBufferError> {
    let mut remaining = self.capacity();

    if points.len() > remaining {
      return Err( BoundedCircularBufferError::InsufficientCapacity );
    }

    /*
    Possibilities:
    r [w - - - - -]
      [r - w - - -]
      [- - r - w -]
      [- - - r - w]
      [w - - - r -]
      [- r w - - -]
     */

    //
    //let mut amount;
    remaining = usize::min( remaining, points.len() );

    // Push into any available tail-space
    if self.write >= self.read {
      //amount = min( N - self.write, remaining );
      //self.buffer[self.write..( self.write + amount )].copy_from_slice( &points[..amount] );
      //remaining -= amount;
    }

    // Push into any available head-space
    if remaining > 0 && self.read < self.write {
      //amount = min( self.read, points.len() );
      //self.buffer[..remaining].copy_from_slice( &points[..amount] );
    }

    //
    //self.write = ( self.write + points.len() ) % N;

    return Ok(());
  }

  pub fn pop_into_slice( &mut self, _points: &mut [T] ) {
    
  }
}
*/