use num::traits::Zero;
use super::{Coord, Size};
use super::DirectionBitmap;

pub trait OutputGrid {
    fn see(&mut self, coord: Coord, bitmap: DirectionBitmap, time: u64);
}

pub trait InputGrid {
    type Opacity;
    type Visibility: Copy
        + Zero
        + PartialOrd<Self::Opacity>
        + PartialOrd<Self::Visibility>
        + ::std::ops::Sub<Self::Opacity, Output = Self::Visibility>;
    fn size(&self) -> Size;
    fn get_opacity(&self, coord: Coord) -> Option<Self::Opacity>;
    fn initial_visibility() -> Self::Visibility;
}
