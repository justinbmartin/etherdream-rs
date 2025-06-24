use std::cmp::min;

pub enum PointBufferError {
  InsufficientCapacity
}

#[repr( C )]
#[derive( Clone, Copy, Debug, Default )]
pub struct Point {
  x: i16,     // x-position
  y: i16,     // y-position
  r: u16,     // red value
  g: u16,     // green value
  b: u16      // blue value
}

impl Point {
  pub fn new( x: i16, y: i16, r: u16, g: u16, b: u16 ) -> Self {
    return Self{ x: x, y: y, r: r, g: g, b: b }
  }
}

#[derive( Debug )]
pub struct PointBuffer<const N: usize> {
  buffer: [Point; N],
  consumer_index: usize,
  producer_index: usize,

}

impl<const N: usize> PointBuffer<N> {
  pub fn new() -> PointBuffer<N> {
    return PointBuffer{
      buffer: [Point::default(); N],
      consumer_index: 0,
      producer_index: 0
    }
  }

  //
  pub fn is_empty( &self ) -> bool { 
    dbg!(self);
    return self.producer_index == self.consumer_index; }

  //
  pub fn is_full( &self ) -> bool { 
    dbg!( self.count() );
    return self.count() == N - 1; }

  //
  pub fn capacity( &self ) -> usize { return self.consumer_index + ( N - self.producer_index ); }

  //
  pub fn count( &self ) -> usize { return ( self.producer_index - self.consumer_index + N ) % N; }

  //
  pub fn push( &mut self, point: Point ) -> bool {
    if self.is_full() {
      return false;
    }

    if self.producer_index > N {
      self.producer_index = 0;
    }

    self.buffer[self.producer_index] = point;
    self.producer_index += 1;
    return true;
  }

  pub fn push_from_slice( &mut self, points: &[Point] ) -> Result<usize,PointBufferError> {
    let remaining = self.capacity();

    if points.len() > remaining {
      return Err(PointBufferError::InsufficientCapacity);
    }

    /*
    Possibilities:
     [* - - - - -]
     [c - p - - -]
     [- - c - p -]
     [- - - c - p]
     [p - - - c -]
     [- - * - - -]
     */

    //
    let mut available = min( self.capacity(), points.len() );

    // Push into any available tail space
    if self.producer_index >= self.consumer_index {
      let amount = min( N - self.producer_index, available );
      self.buffer[self.producer_index..N].copy_from_slice( &points[..amount] );
      available = amount;
    }

    // Push into any available head space
    if available > 0 && self.consumer_index < self.producer_index {
      let amount = min( self.consumer_index, points.len() );
      self.buffer[..self.consumer_index].copy_from_slice( &points[..amount] );
    }

    //
    self.producer_index = ( self.producer_index + available ) % N;
    return Ok( available );
  }

  pub fn pop( &mut self ) -> Option<Point> {
    if self.is_empty() {
      return None;
    }

    let current_index = self.consumer_index;
    self.consumer_index = ( self.consumer_index + 1 ) % N;
    
    return Some( self.buffer[current_index] );
  }

  pub fn pop_into_slice( &mut self, _points: &mut [Point] ) {
    if self.producer_index > self.consumer_index {
      //points[..]
    }
  }
}