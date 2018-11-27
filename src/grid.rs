use coord_2d::{Coord, Size};
use direction::DirectionBitmap;
use num_traits::Zero;
use std::ops::Sub;

pub trait OutputGrid {
    fn see(&mut self, coord: Coord, bitmap: DirectionBitmap, time: u64);
}

pub trait InputGrid {
    type Opacity;
    type Visibility: Copy
        + Zero
        + PartialOrd<Self::Opacity>
        + PartialOrd<Self::Visibility>
        + Sub<Self::Opacity, Output = Self::Visibility>;
    fn size(&self) -> Size;
    fn get_opacity(&self, coord: Coord) -> Option<Self::Opacity>;
    fn initial_visibility() -> Self::Visibility;
}
