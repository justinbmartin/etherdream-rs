mod xs;
mod noop;

pub(crate) use xs::Xs;
pub(crate) use noop::Noop;

pub enum GeneratorId {
  Xs
}

impl GeneratorId {
  pub fn from_str( generator_string: &str ) -> Option<GeneratorId> {
    match generator_string.to_uppercase().as_str() {
      "XS" => Some( GeneratorId::Xs ),
      _ => None
    }
  }
}