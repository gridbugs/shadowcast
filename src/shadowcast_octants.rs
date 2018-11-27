use coord_2d::Coord;
use direction::{Direction, DirectionBitmap};

pub trait Octant {
    fn depth_index(&self, centre: Coord, depth: i32) -> Option<i32>;
    fn make_coord(&self, centre: Coord, lateral_offset: i32, depth_index: i32) -> Coord;
    fn lateral_max(&self, centre: Coord) -> i32;
    fn facing_bitmap(&self) -> DirectionBitmap;
    fn across_bitmap(&self) -> DirectionBitmap;
    fn facing_corner_bitmap(&self) -> DirectionBitmap;
    fn should_see(&self, lateral_offset: i32) -> bool;
}

pub struct TopLeft;
pub struct LeftTop;
pub struct TopRight {
    pub width: i32,
}
pub struct RightTop {
    pub width: i32,
}
pub struct BottomLeft {
    pub height: i32,
}
pub struct LeftBottom {
    pub height: i32,
}
pub struct BottomRight {
    pub width: i32,
    pub height: i32,
}
pub struct RightBottom {
    pub width: i32,
    pub height: i32,
}

macro_rules! some_if {
    ($value:expr, $condition:expr) => {
        if $condition {
            Some($value)
        } else {
            None
        }
    };
}

macro_rules! see_ahead {
    () => {
        fn should_see(&self, _lateral_offset: i32) -> bool { true }
    }
}

macro_rules! no_see_ahead {
    () => {
        fn should_see(&self, lateral_offset: i32) -> bool { lateral_offset != 0 }
    }
}

macro_rules! facing {
    ($dirs:expr) => {
        fn facing_bitmap(&self) -> DirectionBitmap {
            $dirs
        }
    }
}

macro_rules! across {
    ($dirs:expr) => {
        fn across_bitmap(&self) -> DirectionBitmap {
            $dirs
        }
    }
}

macro_rules! facing_corner {
    ($dirs:expr) => {
        fn facing_corner_bitmap(&self) -> DirectionBitmap {
            $dirs
        }
    }
}

impl Octant for TopRight {
    fn depth_index(&self, centre: Coord, depth: i32) -> Option<i32> {
        let index = centre.y - depth;
        some_if!(index, index >= 0)
    }
    fn make_coord(&self, centre: Coord, lateral_offset: i32, depth_index: i32) -> Coord {
        Coord::new(centre.x + lateral_offset, depth_index)
    }
    fn lateral_max(&self, centre: Coord) -> i32 {
        self.width - centre.x - 1
    }
    facing!{Direction::South.bitmap()}
    across!{Direction::West.bitmap()}
    facing_corner!{Direction::SouthWest.bitmap()}
    see_ahead!{}
}

impl Octant for RightTop {
    fn depth_index(&self, centre: Coord, depth: i32) -> Option<i32> {
        let index = centre.x + depth;
        some_if!(index, index < self.width)
    }
    fn make_coord(&self, centre: Coord, lateral_offset: i32, depth_index: i32) -> Coord {
        Coord::new(depth_index, centre.y - lateral_offset)
    }
    fn lateral_max(&self, centre: Coord) -> i32 {
        centre.y
    }
    facing!{Direction::West.bitmap()}
    across!{Direction::South.bitmap()}
    facing_corner!{Direction::SouthWest.bitmap()}
    no_see_ahead!{}
}

impl Octant for TopLeft {
    fn depth_index(&self, centre: Coord, depth: i32) -> Option<i32> {
        let index = centre.y - depth;
        some_if!(index, index >= 0)
    }
    fn make_coord(&self, centre: Coord, lateral_offset: i32, depth_index: i32) -> Coord {
        Coord::new(centre.x - lateral_offset, depth_index)
    }
    fn lateral_max(&self, centre: Coord) -> i32 {
        centre.x
    }
    facing!{Direction::South.bitmap()}
    across!{Direction::East.bitmap()}
    facing_corner!{Direction::SouthEast.bitmap()}
    no_see_ahead!{}
}

impl Octant for LeftTop {
    fn depth_index(&self, centre: Coord, depth: i32) -> Option<i32> {
        let index = centre.x - depth;
        some_if!(index, index >= 0)
    }
    fn make_coord(&self, centre: Coord, lateral_offset: i32, depth_index: i32) -> Coord {
        Coord::new(depth_index, centre.y - lateral_offset)
    }
    fn lateral_max(&self, centre: Coord) -> i32 {
        centre.y
    }
    facing!{Direction::East.bitmap()}
    across!{Direction::South.bitmap()}
    facing_corner!{Direction::SouthEast.bitmap()}
    see_ahead!{}
}

impl Octant for BottomLeft {
    fn depth_index(&self, centre: Coord, depth: i32) -> Option<i32> {
        let index = centre.y + depth;
        some_if!(index, index < self.height)
    }
    fn make_coord(&self, centre: Coord, lateral_offset: i32, depth_index: i32) -> Coord {
        Coord::new(centre.x - lateral_offset, depth_index)
    }
    fn lateral_max(&self, centre: Coord) -> i32 {
        centre.x
    }
    facing!{Direction::North.bitmap()}
    across!{Direction::East.bitmap()}
    facing_corner!{Direction::NorthEast.bitmap()}
    see_ahead!{}
}

impl Octant for LeftBottom {
    fn depth_index(&self, centre: Coord, depth: i32) -> Option<i32> {
        let index = centre.x - depth;
        some_if!(index, index >= 0)
    }
    fn make_coord(&self, centre: Coord, lateral_offset: i32, depth_index: i32) -> Coord {
        Coord::new(depth_index, centre.y + lateral_offset)
    }
    fn lateral_max(&self, centre: Coord) -> i32 {
        self.height - centre.y - 1
    }
    facing!{Direction::East.bitmap()}
    across!{Direction::North.bitmap()}
    facing_corner!{Direction::NorthEast.bitmap()}
    no_see_ahead!{}
}

impl Octant for BottomRight {
    fn depth_index(&self, centre: Coord, depth: i32) -> Option<i32> {
        let index = centre.y + depth;
        some_if!(index, index < self.height)
    }
    fn make_coord(&self, centre: Coord, lateral_offset: i32, depth_index: i32) -> Coord {
        Coord::new(centre.x + lateral_offset, depth_index)
    }
    fn lateral_max(&self, centre: Coord) -> i32 {
        self.width - centre.x - 1
    }
    facing!{Direction::North.bitmap()}
    across!{Direction::West.bitmap()}
    facing_corner!{Direction::NorthWest.bitmap()}
    no_see_ahead!{}
}

impl Octant for RightBottom {
    fn depth_index(&self, centre: Coord, depth: i32) -> Option<i32> {
        let index = centre.x + depth;
        some_if!(index, index < self.width)
    }
    fn make_coord(&self, centre: Coord, lateral_offset: i32, depth_index: i32) -> Coord {
        Coord::new(depth_index, centre.y + lateral_offset)
    }
    fn lateral_max(&self, centre: Coord) -> i32 {
        self.height - centre.y - 1
    }
    facing!{Direction::West.bitmap()}
    across!{Direction::North.bitmap()}
    facing_corner!{Direction::NorthWest.bitmap()}
    see_ahead!{}
}
