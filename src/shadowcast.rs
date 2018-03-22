use std::mem;
use std::cmp;
use num::traits::Zero;
use super::Coord;
use super::DirectionBitmap;
use grid::*;
use shadowcast_octants::*;

#[derive(Debug, Clone, Copy)]
struct Gradient {
    lateral: i32,
    depth: i32,
}

impl Gradient {
    fn new(lateral: i32, depth: i32) -> Self {
        Self { lateral, depth }
    }
}

struct StaticParams<'a, In: 'a + InputGrid> {
    centre: Coord,
    vision_distance_squared: i32,
    time: u64,
    input_grid: &'a In,
    width: i32,
    height: i32,
}

#[derive(Clone, Debug)]
struct ScanParams<Visibility> {
    min_gradient: Gradient,
    max_gradient: Gradient,
    depth: i32,
    visibility: Visibility,
}

impl<Visibility> ScanParams<Visibility> {
    fn with_visibility(visibility: Visibility) -> Self {
        Self {
            min_gradient: Gradient::new(0, 1),
            max_gradient: Gradient::new(1, 1),
            depth: 1,
            visibility,
        }
    }
}

struct CornerInfo {
    bitmap: DirectionBitmap,
    coord: Coord,
}

fn scan<Out, In, O>(
    output_grid: &mut Out,
    octant: &O,
    next: &mut Vec<ScanParams<In::Visibility>>,
    params: ScanParams<In::Visibility>,
    static_params: &StaticParams<In>,
) -> Option<CornerInfo>
where
    Out: OutputGrid,
    In: InputGrid,
    O: Octant,
    In::Visibility: Copy
        + Zero
        + PartialOrd<In::Opacity>
        + PartialOrd<In::Visibility>
        + ::std::ops::Sub<In::Opacity, Output = In::Visibility>,
{
    let ScanParams {
        mut min_gradient,
        max_gradient,
        depth,
        visibility,
    } = params;

    let depth_index = if let Some(depth_index) = octant.depth_index(static_params.centre, depth) {
        depth_index
    } else {
        return None;
    };

    let front_gradient_depth = depth * 2 - 1;
    let back_gradient_depth = front_gradient_depth + 2;

    let double_start_num = min_gradient.depth + front_gradient_depth * min_gradient.lateral;
    let double_stop_num = max_gradient.depth + back_gradient_depth * max_gradient.lateral;

    let lateral_min = double_start_num / (2 * min_gradient.depth);

    let stop_denom = 2 * max_gradient.depth;
    let lateral_max = if double_stop_num % stop_denom == 0 {
        (double_stop_num - 1) / stop_denom
    } else {
        double_stop_num / stop_denom
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

        if cur_opaque {
            // check if we can actually see the facing side
            if max_gradient.lateral * front_gradient_depth > gradient_lateral * max_gradient.depth {
                direction_bitmap |= octant.facing_bitmap();
            } else {
                direction_bitmap |= octant.facing_corner_bitmap();
            }
        } else {
            direction_bitmap |= DirectionBitmap::all();
        };

        // handle changes in opacity
        if lateral_index != lateral_min && cur_visibility != prev_visibility {
            // use the back of the cell if necessary
            let gradient_depth = if cur_visibility < prev_visibility {
                back_gradient_depth
            } else {
                front_gradient_depth
            };
            let gradient = Gradient::new(gradient_lateral, gradient_depth);

            if !prev_opaque && in_range {
                // see beyond the previous section unless it's opaque
                next.push(ScanParams {
                    min_gradient,
                    max_gradient: gradient,
                    depth: depth + 1,
                    visibility: prev_visibility,
                });
            }

            min_gradient = gradient;
            if cur_opaque {
                // the edge of the current cell is visible through the previous cell
                direction_bitmap |= octant.across_bitmap();
            }
        }

        // handle final cell
        if lateral_index == lateral_max {
            if !cur_opaque && in_range {
                // see beyond the current section
                next.push(ScanParams {
                    min_gradient,
                    max_gradient,
                    depth: depth + 1,
                    visibility: cur_visibility,
                });
            }
            if in_range && lateral_index == depth {
                return Some(CornerInfo {
                    bitmap: direction_bitmap,
                    coord,
                });
            }
        }

        if in_range && octant.should_see(lateral_index) {
            output_grid.see(coord, direction_bitmap, static_params.time);
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

    fn observe_octant<Out, In, A, B>(
        &mut self,
        output_grid: &mut Out,
        octant_a: A,
        octant_b: B,
        static_params: &StaticParams<In>,
    ) where
        Out: OutputGrid,
        In: InputGrid<Visibility = Visibility>,
        In::Visibility: Copy
            + Zero
            + PartialOrd<In::Opacity>
            + PartialOrd<In::Visibility>
            + ::std::ops::Sub<In::Opacity, Output = In::Visibility>,
        A: Octant,
        B: Octant,
    {
        self.queue_a
            .push(ScanParams::with_visibility(In::initial_visibility()));
        self.queue_b
            .push(ScanParams::with_visibility(In::initial_visibility()));

        loop {
            let mut corner_bitmap = DirectionBitmap::empty();
            let mut corner_coord = None;

            while let Some(params) = self.queue_a.pop() {
                if let Some(corner) = scan(
                    output_grid,
                    &octant_a,
                    &mut self.queue_a_swap,
                    params,
                    static_params,
                ) {
                    corner_bitmap |= corner.bitmap;
                    corner_coord = Some(corner.coord);
                }
            }

            while let Some(params) = self.queue_b.pop() {
                if let Some(corner) = scan(
                    output_grid,
                    &octant_b,
                    &mut self.queue_b_swap,
                    params,
                    static_params,
                ) {
                    corner_bitmap |= corner.bitmap;
                }
            }

            if let Some(corner_coord) = corner_coord {
                output_grid.see(corner_coord, corner_bitmap, static_params.time);
            }

            if self.queue_a_swap.is_empty() && self.queue_b_swap.is_empty() {
                break;
            }
            mem::swap(&mut self.queue_a, &mut self.queue_a_swap);
            mem::swap(&mut self.queue_b, &mut self.queue_b_swap);
        }
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
            + Zero
            + PartialOrd<In::Opacity>
            + PartialOrd<In::Visibility>
            + ::std::ops::Sub<In::Opacity, Output = In::Visibility>,
    {
        output_grid.see(coord, DirectionBitmap::all(), time);

        let size = input_grid.size();
        let width = size.x() as i32;
        let height = size.y() as i32;

        let params = StaticParams {
            centre: coord,
            vision_distance_squared: (distance * distance) as i32,
            time,
            input_grid,
            width,
            height,
        };

        self.observe_octant(output_grid, TopLeft, LeftTop, &params);
        self.observe_octant(output_grid, TopRight { width }, RightTop { width }, &params);
        self.observe_octant(
            output_grid,
            BottomLeft { height },
            LeftBottom { height },
            &params,
        );
        self.observe_octant(
            output_grid,
            BottomRight { width, height },
            RightBottom { width, height },
            &params,
        );
    }
}
