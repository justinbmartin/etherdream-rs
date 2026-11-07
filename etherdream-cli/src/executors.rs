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

pub fn make_sequencer( client: etherdream::Client ) -> Sequencer<Track> {
  let mut sequencer = Sequencer::<Track>::new( client );
  sequencer.add_track( TrackDefinition::new( Track::Beam, ||{ Box::new( beam::Beam::default() ) }) );
  sequencer.add_track( TrackDefinition::new( Track::Windmill, ||{ Box::new( windmill::Windmill::new() ) }) );

  sequencer
}