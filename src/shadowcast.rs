use coord_2d::Coord;
use direction::DirectionBitmap;
use grid::*;
use num_traits::Zero;
use shadowcast_octants::*;
use std::cmp;
use std::mem;
use std::ops::Sub;

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

struct StaticParams<'a, In: 'a + InputGrid> {
    centre: Coord,
    vision_distance_squared: i32,
    input_grid: &'a In,
    width: i32,
    height: i32,
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

struct CornerInfo {
    bitmap: DirectionBitmap,
    coord: Coord,
}

fn scan<In, O, F>(
    octant: &O,
    next: &mut Vec<ScanParams<In::Visibility>>,
    params: ScanParams<In::Visibility>,
    static_params: &StaticParams<In>,
    f: &mut F,
) -> Option<CornerInfo>
where
    F: FnMut(Coord, DirectionBitmap),
    In: InputGrid,
    O: Octant,
    In::Visibility: Copy
        + ::std::fmt::Debug
        + Zero
        + PartialOrd<In::Opacity>
        + PartialOrd<In::Visibility>
        + Sub<In::Opacity, Output = In::Visibility>,
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
        // We're interested in the width in half-cells of the right triangle which is similar to
        // min_gradient, and whose depth is effective_gradient_depth. Since the eye is in the centre of a
        // cell, the lateral min index will be half of (this width + 1).
        // It's incremented to account for the eye being in the centre of its cell (ie. 1 half-cell
        // to the right of the left edge of the cell.
        // It's halved because the computed width will be in half-cells.
        //
        // Similar triangles:
        // width_half_cells / effective_gradient_depth = min_gradient.lateral / min_gradient.depth
        //
        // Thus:
        // width_half_cells = (min_gradient.lateral * effective_gradient_depth) / min_gradient.depth
        //
        // Since the eye is in the centre of a cell:
        // offset_half_cells = 1 + width_half_cells
        //                   = 1 + ((min_gradient.lateral * effective_gradient_depth) / min_gradient.depth)
        //                   = (min_gradient_depth + (min_gradient.lateral * effectivte_gradient_depth)) /
        //                     min_gradient.depth
        //
        // So the offset in cells is:
        // offset_cells = offset_half_cells / 2
        //              = (min_gradient_depth + (min_gradient.lateral * effective_gradient_depth)) /
        //                (min_gradient.depth * 2)
        //
        // Finally, if this section is not min_inclusive, we skip the first index, increment
        // the result by 1.
        ((min_gradient.depth + (min_gradient.lateral * effective_gradient_depth))
            / (min_gradient.depth * 2)) + ((!min_inclusive) as i32)
    };

    let lateral_max = {
        // This computation is much the same as for lateral_min above. Notable differences:
        // - subtract 1 before dividing, to make sure that if the strip ends exactly on a left
        //   corner of a cell, that cell is not included in the scanned range
        // - there is no max_inclusive analog of min_inclusive. All ranges are effectively
        //   max inclusive, so there is no need to change the result accordingly
        (max_gradient.depth + (max_gradient.lateral * effective_gradient_depth) - 1)
            / (max_gradient.depth * 2)
    };

    let lateral_max = cmp::min(lateral_max, octant.lateral_max(static_params.centre));

    let mut prev_visibility = Zero::zero();
    let mut prev_opaque = false;

    for lateral_index in lateral_min..(lateral_max + 1) {
        let coord = octant.make_coord(static_params.centre, lateral_index, depth_index);
        if coord.x < 0 || coord.x >= static_params.width || coord.y < 0
            || coord.y >= static_params.height
        {
            break;
        };

        let opacity = if let Some(opacity) = static_params.input_grid.get_opacity(coord) {
            opacity
        } else {
            break;
        };

        // check if cell is in visible range
        let between = coord - static_params.centre;
        let distance_squared = between.x * between.x + between.y * between.y;
        let in_range = distance_squared < static_params.vision_distance_squared;

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
            if !prev_opaque && in_range {
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
            if !cur_opaque && in_range && min_gradient != max_gradient {
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
                });
            }
        }

        if in_range && octant.should_see(lateral_index) {
            f(coord, direction_bitmap);
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

    fn observe_octant<In, A, B, F>(
        &mut self,
        octant_a: A,
        octant_b: B,
        static_params: &StaticParams<In>,
        f: &mut F,
    ) where
        F: FnMut(Coord, DirectionBitmap),
        In: InputGrid<Visibility = Visibility>,
        In::Visibility: Copy
            + ::std::fmt::Debug
            + Zero
            + PartialOrd<In::Opacity>
            + PartialOrd<In::Visibility>
            + Sub<In::Opacity, Output = In::Visibility>,
        A: Octant,
        B: Octant,
    {
        self.queue_a
            .push(ScanParams::octant_base(In::initial_visibility()));
        self.queue_b
            .push(ScanParams::octant_base(In::initial_visibility()));

        loop {
            let mut corner_bitmap = DirectionBitmap::empty();
            let mut corner_coord = None;

            for params in self.queue_a.drain(..) {
                if let Some(corner) = scan(
                    &octant_a,
                    &mut self.queue_a_swap,
                    params,
                    static_params,
                    f,
                ) {
                    corner_bitmap |= corner.bitmap;
                    corner_coord = Some(corner.coord);
                }
            }

            for params in self.queue_b.drain(..) {
                if let Some(corner) = scan(
                    &octant_b,
                    &mut self.queue_b_swap,
                    params,
                    static_params,
                    f,
                ) {
                    corner_bitmap |= corner.bitmap;
                    corner_coord = Some(corner.coord);
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
                f(corner_coord, corner_bitmap);
            }

            if self.queue_a_swap.is_empty() && self.queue_b_swap.is_empty() {
                break;
            }
            mem::swap(&mut self.queue_a, &mut self.queue_a_swap);
            mem::swap(&mut self.queue_b, &mut self.queue_b_swap);
        }
    }

    pub fn for_each<F, In>(
        &mut self,
        coord: Coord,
        input_grid: &In,
        distance: u32,
        mut f: F,
    ) where
        In: InputGrid<Visibility = Visibility>,
        In::Visibility: Copy
            + ::std::fmt::Debug
            + Zero
            + PartialOrd<In::Opacity>
            + PartialOrd<In::Visibility>
            + Sub<In::Opacity, Output = In::Visibility>,
        F: FnMut(Coord, DirectionBitmap),
    {
        f(coord, DirectionBitmap::all());
        let size = input_grid.size();
        let width = size.x() as i32;
        let height = size.y() as i32;
        let params = StaticParams {
            centre: coord,
            vision_distance_squared: (distance * distance) as i32,
            input_grid,
            width,
            height,
        };
        self.observe_octant(TopLeft, LeftTop, &params, &mut f);
        self.observe_octant(
            TopRight { width },
            RightTop { width },
            &params,
            &mut f,
        );
        self.observe_octant(
            BottomLeft { height },
            LeftBottom { height },
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

    pub fn observe<Out, In>(
        &mut self,
        coord: Coord,
        input_grid: &In,
        distance: u32,
        time: u64,
        output_grid: &mut Out,
    ) where
        Out: OutputGrid,
        In: InputGrid<Visibility = Visibility>,
        In::Visibility: Copy
            + ::std::fmt::Debug
            + Zero
            + PartialOrd<In::Opacity>
            + PartialOrd<In::Visibility>
            + Sub<In::Opacity, Output = In::Visibility>,
    {
        self.for_each(coord, input_grid, distance, |coord, direction_map| {
            output_grid.see(coord, direction_map, time);
        });
    }
}
