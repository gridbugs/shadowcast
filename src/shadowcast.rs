use coord_2d::{Coord, Size};
use direction::DirectionBitmap;
use num_traits::Zero;
use octants::*;
use std::cmp;
use std::mem;
use std::ops::Sub;

pub trait InputGrid {
    /// Representation of the opacity of a cell in the grid.
    /// This will usually be a numeric type.
    type Opacity;

    /// Returns the size of the grid in cells.
    fn size(&self) -> Size;

    /// Returns the opacity at a given coordinate. This method may panic
    /// the coord lies out of the bounds described by `size`. The contract
    /// implemented by `ShadowcastContext::for_each` includes not calling
    /// this with an out-of-bounds coordinate.
    fn get_opacity(&self, coord: Coord) -> Self::Opacity;
}

pub trait VisionDistance {
    fn in_range(&self, delta: Coord) -> bool;
}

pub mod vision_distance {
    use super::VisionDistance;
    use coord_2d::Coord;
    use std::cmp;

    #[derive(Debug, Clone, Copy)]
    pub struct Circle {
        distance_squared: u32,
    }

    impl Circle {
        pub fn new(distance: u32) -> Self {
            Self {
                distance_squared: distance * distance,
            }
        }
    }

    impl VisionDistance for Circle {
        fn in_range(&self, delta: Coord) -> bool {
            ((delta.x * delta.x + delta.y * delta.y) as u32) <= self.distance_squared
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Square {
        distance: u32,
    }

    impl Square {
        pub fn new(distance: u32) -> Self {
            Self { distance }
        }
    }

    impl VisionDistance for Square {
        fn in_range(&self, delta: Coord) -> bool {
            cmp::max(delta.x.abs(), delta.y.abs()) as u32 <= self.distance
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Diamond {
        distance: u32,
    }

    impl Diamond {
        pub fn new(distance: u32) -> Self {
            Self { distance }
        }
    }

    impl VisionDistance for Diamond {
        fn in_range(&self, delta: Coord) -> bool {
            ((delta.x.abs() + delta.y.abs()) as u32) <= self.distance
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Gradient {
    lateral: i32,
    depth: i32,
}

impl PartialEq for Gradient {
    fn eq(&self, other: &Self) -> bool {
        self.lateral * other.depth == self.depth * other.lateral
    }
}

impl Gradient {
    fn new(lateral: i32, depth: i32) -> Self {
        Self { lateral, depth }
    }
}

struct StaticParams<'a, In: 'a + InputGrid, Visibility, VisDist> {
    centre: Coord,
    vision_distance: VisDist,
    input_grid: &'a In,
    width: i32,
    height: i32,
    initial_visibility: Visibility,
}

#[derive(Clone, Debug)]
struct ScanParams<Visibility> {
    min_gradient: Gradient,
    max_gradient: Gradient,
    min_inclusive: bool,
    depth: i32,
    visibility: Visibility,
}

impl<Visibility> ScanParams<Visibility> {
    fn octant_base(visibility: Visibility) -> Self {
        Self {
            min_gradient: Gradient::new(0, 1),
            max_gradient: Gradient::new(1, 1),
            min_inclusive: true,
            depth: 1,
            visibility,
        }
    }
}

struct CornerInfo<Visibility> {
    bitmap: DirectionBitmap,
    coord: Coord,
    visibility: Visibility,
}

fn scan<In, Visibility, O, VisDist, F>(
    octant: &O,
    next: &mut Vec<ScanParams<Visibility>>,
    params: ScanParams<Visibility>,
    static_params: &StaticParams<In, Visibility, VisDist>,
    f: &mut F,
) -> Option<CornerInfo<Visibility>>
where
    In: InputGrid,
    O: Octant,
    Visibility: Copy
        + Zero
        + PartialOrd<In::Opacity>
        + PartialOrd
        + Sub<In::Opacity, Output = Visibility>,
    VisDist: VisionDistance,
    F: FnMut(Coord, DirectionBitmap, Visibility),
{
    let ScanParams {
        mut min_gradient,
        max_gradient,
        mut min_inclusive,
        depth,
        visibility,
    } = params;

    let depth_index =
        if let Some(depth_index) = octant.depth_index(static_params.centre, depth) {
            depth_index
        } else {
            // depth puts this strip out of bounds within the current octant
            return None;
        };

    // the distance in half-cells between the centre of the row being scanned
    // and the centre of the eye
    let mid_gradient_depth = depth * 2;
    let front_gradient_depth = mid_gradient_depth - 1;
    let back_gradient_depth = mid_gradient_depth + 1;

    let effective_gradient_depth = mid_gradient_depth;

    let lateral_min = {
        // We're interested in the width in half-cells of the right triangle which is
        // similar to min_gradient, and whose depth is effective_gradient_depth. Since the
        // eye is in the centre of a cell, the lateral min index will be half of (this
        // width + 1).  It's incremented to account for the eye being in the centre of its
        // cell (ie. 1 half-cell to the right of the left edge of the cell.  It's halved
        // because the computed width will be in half-cells.
        //
        // Similar triangles:
        // width_half_cells / effective_gradient_depth =
        // min_gradient.lateral / min_gradient.depth
        //
        // Thus:
        // width_half_cells = (min_gradient.lateral * effective_gradient_depth) /
        //                    min_gradient.depth
        //
        // Since the eye is in the centre of a cell:
        // offset_half_cells = 1 + width_half_cells
        //                   = 1 + ((min_gradient.lateral * effective_gradient_depth) /
        //                         min_gradient.depth)
        //                   = (min_gradient_depth + (min_gradient.lateral *
        //                      effectivte_gradient_depth)) / min_gradient.depth
        //
        // So the offset in cells is:
        // offset_cells = offset_half_cells / 2
        //              = (min_gradient_depth +
        //                      (min_gradient.lateral * effective_gradient_depth)) /
        //                (min_gradient.depth * 2)
        //
        // Finally, if this section is not min_inclusive, we skip the first index,
        // increment the result by 1.
        ((min_gradient.depth + (min_gradient.lateral * effective_gradient_depth))
            / (min_gradient.depth * 2))
            + ((!min_inclusive) as i32)
    };

    let lateral_max = {
        // This computation is much the same as for lateral_min above. Notable
        // differences: - subtract 1 before dividing, to make sure that if the strip ends
        // exactly on a left corner of a cell, that cell is not included in the scanned
        // range - there is no max_inclusive analog of min_inclusive. All ranges are
        // effectively max inclusive, so there is no need to change the result accordingly
        (max_gradient.depth + (max_gradient.lateral * effective_gradient_depth) - 1)
            / (max_gradient.depth * 2)
    };

    // prevent scanning off the edge of the octant
    let lateral_max = cmp::min(lateral_max, octant.lateral_max(static_params.centre));

    let mut prev_visibility = Zero::zero();
    let mut prev_opaque = false;

    for lateral_index in lateral_min..(lateral_max + 1) {
        let coord = octant.make_coord(static_params.centre, lateral_index, depth_index);
        if coord.x < 0
            || coord.x >= static_params.width
            || coord.y < 0
            || coord.y >= static_params.height
        {
            break;
        };

        let opacity = static_params.input_grid.get_opacity(coord);

        // check if cell is in visible range
        let in_range = static_params
            .vision_distance
            .in_range(coord - static_params.centre);

        let gradient_lateral = lateral_index * 2 - 1;
        let mut direction_bitmap = DirectionBitmap::empty();

        let (cur_visibility, cur_opaque) = if visibility > opacity {
            (visibility - opacity, false)
        } else {
            (Zero::zero(), true)
        };

        // handle changes in opacity
        if lateral_index != lateral_min && cur_visibility != prev_visibility {
            let gradient_depth = if cur_visibility < prev_visibility {
                // getting more opaque
                back_gradient_depth
            } else {
                // getting less opaque
                front_gradient_depth
            };
            let gradient = Gradient::new(gradient_lateral, gradient_depth);
            if !prev_opaque {
                // see beyond the previous section unless it's opaque
                next.push(ScanParams {
                    min_gradient,
                    max_gradient: gradient,
                    min_inclusive,
                    depth: depth + 1,
                    visibility: prev_visibility,
                });
            }
            min_gradient = gradient;
            min_inclusive = false;
            // If the current cell is opaque, then the previous cell was not opaque and so
            // we can see the across edge through the previous cell.
            // If the current cell is transparent, we can see the entire cell (including
            // the across edge), so setting it again here doesn't hurt.
            direction_bitmap |= octant.across_bitmap();
        }
        if cur_opaque {
            // check if we can actually see the facing side
            if max_gradient.lateral * front_gradient_depth
                > gradient_lateral * max_gradient.depth
            {
                direction_bitmap |= octant.facing_bitmap();
            } else if direction_bitmap.is_empty() {
                // only set the corner as visible if no edge is already visible
                direction_bitmap |= octant.facing_corner_bitmap();
            }
        } else {
            direction_bitmap |= DirectionBitmap::all();
        };

        // handle final cell
        if lateral_index == lateral_max {
            if !cur_opaque && min_gradient != max_gradient {
                // see beyond the current section
                next.push(ScanParams {
                    min_gradient,
                    max_gradient,
                    min_inclusive,
                    depth: depth + 1,
                    visibility: cur_visibility,
                });
            }
            if in_range && lateral_index == depth {
                // Intentionally don't invoke the callback on the final cell of
                // the scan, if it's along the diagonal between two octants.
                // The result of both octant scans is required to determine the
                // visibility of this cell. It is handled in
                // ShadowcastContext::observe_octant.
                return Some(CornerInfo {
                    bitmap: direction_bitmap,
                    coord,
                    visibility,
                });
            }
        }

        if in_range && octant.should_see(lateral_index) {
            f(coord, direction_bitmap, visibility);
        }

        prev_visibility = cur_visibility;
        prev_opaque = cur_opaque;
    }

    None
}

#[derive(Clone, Debug)]
pub struct ShadowcastContext<Visibility> {
    queue_a: Vec<ScanParams<Visibility>>,
    queue_a_swap: Vec<ScanParams<Visibility>>,
    queue_b: Vec<ScanParams<Visibility>>,
    queue_b_swap: Vec<ScanParams<Visibility>>,
}

impl<Visibility> ShadowcastContext<Visibility> {
    pub fn new() -> Self {
        Self {
            queue_a: Vec::new(),
            queue_a_swap: Vec::new(),
            queue_b: Vec::new(),
            queue_b_swap: Vec::new(),
        }
    }

    fn observe_octant<In, A, B, VisDist, F>(
        &mut self,
        octant_a: A,
        octant_b: B,
        static_params: &StaticParams<In, Visibility, VisDist>,
        f: &mut F,
    ) where
        In: InputGrid,
        Visibility: Copy
            + Zero
            + PartialOrd<In::Opacity>
            + PartialOrd
            + Sub<In::Opacity, Output = Visibility>,
        A: Octant,
        B: Octant,
        VisDist: VisionDistance,
        F: FnMut(Coord, DirectionBitmap, Visibility),
    {
        self.queue_a
            .push(ScanParams::octant_base(static_params.initial_visibility));
        self.queue_b
            .push(ScanParams::octant_base(static_params.initial_visibility));

        loop {
            let mut corner_bitmap = DirectionBitmap::empty();
            let mut corner_coord = None;
            let mut corner_visibility = Zero::zero();

            for params in self.queue_a.drain(..) {
                if let Some(corner) =
                    scan(&octant_a, &mut self.queue_a_swap, params, static_params, f)
                {
                    corner_bitmap |= corner.bitmap;
                    corner_coord = Some(corner.coord);
                    if corner.visibility > corner_visibility {
                        corner_visibility = corner.visibility;
                    }
                }
            }

            for params in self.queue_b.drain(..) {
                if let Some(corner) =
                    scan(&octant_b, &mut self.queue_b_swap, params, static_params, f)
                {
                    corner_bitmap |= corner.bitmap;
                    corner_coord = Some(corner.coord);
                    if corner.visibility > corner_visibility {
                        corner_visibility = corner.visibility;
                    }
                }
            }

            if let Some(corner_coord) = corner_coord {
                if !(corner_bitmap.is_full()
                    || (corner_bitmap & DirectionBitmap::all_cardinal()).is_empty())
                {
                    // if one of the scans saw a corner only but the other saw
                    // the entire edge, just keep the edge.
                    corner_bitmap &= DirectionBitmap::all_cardinal();
                }
                f(corner_coord, corner_bitmap, corner_visibility);
            }

            if self.queue_a_swap.is_empty() && self.queue_b_swap.is_empty() {
                break;
            }
            mem::swap(&mut self.queue_a, &mut self.queue_a_swap);
            mem::swap(&mut self.queue_b, &mut self.queue_b_swap);
        }
    }

    pub fn for_each<F, In, VisDist>(
        &mut self,
        coord: Coord,
        input_grid: &In,
        vision_distance: VisDist,
        initial_visibility: Visibility,
        mut f: F,
    ) where
        In: InputGrid,
        Visibility: Copy
            + Zero
            + PartialOrd<In::Opacity>
            + PartialOrd
            + Sub<In::Opacity, Output = Visibility>,
        VisDist: VisionDistance,
        F: FnMut(Coord, DirectionBitmap, Visibility),
    {
        f(coord, DirectionBitmap::all(), initial_visibility);
        let size = input_grid.size();
        let width = size.x() as i32;
        let height = size.y() as i32;
        let params = StaticParams {
            centre: coord,
            vision_distance,
            input_grid,
            width,
            height,
            initial_visibility,
        };
        self.observe_octant(TopLeft, LeftTop, &params, &mut f);
        self.observe_octant(RightTop { width }, TopRight { width }, &params, &mut f);
        self.observe_octant(
            LeftBottom { height },
            BottomLeft { height },
            &params,
            &mut f,
        );
        self.observe_octant(
            BottomRight { width, height },
            RightBottom { width, height },
            &params,
            &mut f,
        );
    }
}
