extern crate coord_2d;
extern crate direction;
extern crate num_traits;
#[cfg(feature = "serialize")]
#[macro_use]
extern crate serde;

mod octants;
mod shadowcast;

pub use shadowcast::*;

#[cfg(test)]
mod test;
