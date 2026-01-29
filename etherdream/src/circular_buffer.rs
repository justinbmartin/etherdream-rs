//! A bounded single-producer single-consumer (SPSC) circular buffer.
use std::mem::ManuallyDrop;
use std::sync::{ Arc, atomic::{ AtomicUsize, Ordering } };

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Circular Buffer

pub struct CircularBuffer<T> {
  capacity: usize,
  data: *mut T,
  size: AtomicUsize
}

impl<T> CircularBuffer<T>
  where T: Copy + Clone + Default + Send + Sync
{
  /// Creates a new single-producer single-consumer (SPSC) bounded circular buffer.
  pub fn new( capacity: usize ) -> ( Writer<T>, Reader<T> )
  {
    let buffer = Arc::new( CircularBuffer::<T>{
      capacity,
      data: ManuallyDrop::new( vec![ T::default(); capacity ] ).as_mut_ptr(),
      size: AtomicUsize::new( 0 )
    });

    (
      Writer::<T>{ buffer: buffer.clone(), capacity, head: 0 },
      Reader::<T>{ buffer: buffer.clone(), capacity, tail: 0 }
    )
  }
}

impl<T> Drop for CircularBuffer<T> {
  fn drop( &mut self ) {
    // SAFETY: Compliment to the `ManuallyDrop` that wraps `data`.
    unsafe { drop( Vec::from_raw_parts( self.data, self.capacity, self.capacity ) ) };
  }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Reader

pub struct Reader<T>
{
  buffer: Arc<CircularBuffer<T>>,
  capacity: usize,
  tail: usize
}

impl<T> Reader<T>
{
  /// Returns true if the buffer is empty
  pub fn is_empty( &self ) -> bool {
    self.len() == 0
  }

  /// Returns true if the buffer is at capacity
  pub fn is_full( &self ) -> bool { 
    self.len() == self.capacity
  }

  /// Returns the count of unread items in the buffer
  pub fn len( &self ) -> usize {
    self.buffer.size.load( Ordering::Acquire )
  }

  /// Returns the remaining capacity in the buffer
  pub fn remaining( &self ) -> usize {
    self.capacity - self.len()
  }

  /// Returns the static maximum capacity of the buffer
  pub fn capacity( &self ) -> usize {
    self.capacity
  }

  /// Returns the currently available item (if one is available) and advances
  /// the head. If the buffer is empty, returns None.
  pub fn pop( &mut self ) -> Option<T> {
    if self.is_empty() {
      return None;
    }

    let item = unsafe{ self.buffer.data.add( self.tail ).read() };
    self.tail = ( self.tail + 1 ) % self.capacity;
    self.buffer.size.fetch_sub( 1, Ordering::AcqRel );
    Some( item )
  }
}

// SAFETY:
// * Compile-time guarantee that there is only one writer to the shared buffer.
// * Logical guarantee that the writer will never access the reader's tail.
unsafe impl<T: Send> Send for Reader<T> { }
unsafe impl<T: Sync> Sync for Reader<T> { }

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - Writer

pub struct Writer<T>
{
  buffer: Arc<CircularBuffer<T>>,
  capacity: usize,
  head: usize
}

impl<T> Writer<T>
{
  /// Returns true if the buffer is empty
  pub fn is_empty( &self ) -> bool {
    self.len() == 0
  }

  /// Returns true if the buffer is at capacity
  pub fn is_full( &self ) -> bool { 
    self.len() == self.capacity
  }

  /// Returns the count of unread items in the buffer
  pub fn len( &self ) -> usize {
    self.buffer.size.load( Ordering::Acquire )
  }

  /// Returns the remaining capacity in the buffer
  pub fn remaining( &self ) -> usize {
    self.capacity - self.len()
  }

  /// Returns the static maximum capacity of the buffer
  pub fn capacity( &self ) -> usize {
    self.capacity
  }

  /// Pushes `item` into the buffer (if space is available) and advances the
  /// tail. If the buffer is full, returns the `item` as `Some( item )`.
  pub fn push( &mut self, item: T ) -> Option<T> {
    if self.is_full() {
      return Some( item );
    }

    unsafe{ self.buffer.data.add( self.head ).write( item ); }
    self.head = ( self.head + 1 ) % self.capacity;
    self.buffer.size.fetch_add( 1, Ordering::AcqRel );
    None
  }
}

// SAFETY:
// * Compile-time guarantee that there is only one reader to the shared buffer.
// * Logical guarantee that the reader will never consume from the writer's head.
unsafe impl<T: Send> Send for Writer<T> { }
unsafe impl<T: Sync> Sync for Writer<T> { }