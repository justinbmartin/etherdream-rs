use std::fmt;

use crate::sequencer::{ Sequencer, TrackDefinition };

mod beam;
mod common;
mod windmill;

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  Track Identifiers

#[derive( Clone, Copy, Debug, Eq, Hash, PartialEq )]
pub enum Track {
  Beam,
  Windmill
}

impl fmt::Display for Track {
  fn fmt( &self, f: &mut fmt::Formatter<'_> ) -> fmt::Result {
    match self {
      Self::Beam => write!( f, "Beam" ),
      Self::Windmill => write!( f, "Windmill" )
    }
  }
}

pub fn make_sequencer( client: etherdream::Client ) -> Sequencer<Track> {
  let tracks: Vec<TrackDefinition<Track>> = vec![
    TrackDefinition::new( Track::Beam, ||{ Box::new( beam::Beam::default() ) }),
    TrackDefinition::new( Track::Windmill, ||{ Box::new( windmill::Windmill::new() ) })
  ];

  Sequencer::<Track>::new( client, tracks )
}