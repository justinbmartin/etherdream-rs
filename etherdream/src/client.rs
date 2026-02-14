use crate::conn;

pub struct Client {
  conn: conn::Conn
}

impl Client {
  pub fn new( conn: conn::Conn ) -> Self {
    Client{ conn }
  }

  #[inline]
  pub fn push_point( &mut self, x: i16, y: i16, r: u16, g: u16, b: u16 ) -> Option<conn::PointData> {
    self.conn.push_point( x, y, r, g, b )
  }

  #[inline]
  pub async fn start( &mut self, rate: usize ) -> conn::CommandResult {
    self.conn.start( rate ).await
  }
}