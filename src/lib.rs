extern crate coord_2d;
extern crate direction;
extern crate num_traits;

mod octants;
mod shadowcast;

pub use shadowcast::*;

#[cfg(test)]
mod test;
