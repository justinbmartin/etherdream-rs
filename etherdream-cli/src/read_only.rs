use std::sync::Arc;

use tokio::sync::{ RwLock, RwLockReadGuard };

/// A read-only ARC wrapper for T's
pub struct ReadOnly<T> {
  inner: Arc<RwLock<T>>
}

impl<T> ReadOnly<T> {
  pub fn new( item: Arc<RwLock<T>> ) -> Self {
    Self{ inner: item }
  }

  // Will panic if called in an async context.
  pub fn blocking_read( &'_ self ) -> RwLockReadGuard<'_, T> {
    self.inner.blocking_read()
  }
}

impl<T> Clone for ReadOnly<T> {
  /// Creates a new ReadOnly<T> that clones the inner item.
  fn clone( &self ) -> Self {
    Self{ inner: self.inner.clone() }
  }
}