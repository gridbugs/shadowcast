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
    max_inclusive: bool,
    depth: i32,
    visibility: Visibility,
}

impl<Visibility> ScanParams<Visibility> {
    fn octant_base(visibility: Visibility, min_inclusive: bool, max_inclusive: bool) -> Self {
        Self {
            min_gradient: Gradient::new(0, 1),
            max_gradient: Gradient::new(1, 1),
            min_inclusive,
            max_inclusive,
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
        + Zero
        + PartialOrd<In::Opacity>
        + PartialOrd<In::Visibility>
        + Sub<In::Opacity, Output = In::Visibility>,
{
    let ScanParams {
        mut min_gradient,
        max_gradient,
        mut min_inclusive,
        max_inclusive,
        depth,
        visibility,
    } = params;

    let depth_index = if let Some(depth_index) = octant.depth_index(static_params.centre, depth) {
        depth_index
    } else {
        // depth puts this strip out of bounds within the current octant
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

    println!(
        "{:?} {}     {:?} {}",
        lateral_min, min_inclusive, lateral_max, max_inclusive
    );

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
            let (gradient_depth, inclusive) = if cur_visibility < prev_visibility {
                // getting more opaque
                (back_gradient_depth, false)
            } else {
                // getting less opaque
                (front_gradient_depth, true)
            };
            let gradient = Gradient::new(gradient_lateral, gradient_depth);
            if !prev_opaque && in_range {
                // see beyond the previous section unless it's opaque
                next.push(ScanParams {
                    min_gradient,
                    max_gradient: gradient,
                    min_inclusive,
                    max_inclusive: inclusive,
                    depth: depth + 1,
                    visibility: prev_visibility,
                });
            }
            min_gradient = gradient;
            min_inclusive = !inclusive;
            // If the current cell is opaque, then the previous cell was not opaque and so
            // we can see the across edge through the previous cell.
            // If the current cell is transparent, we can see the entire cell (including
            // the across edge), so setting it again here doesn't hurt.
            direction_bitmap |= octant.across_bitmap();
        }
        if cur_opaque {
            // check if we can actually see the facing side
            if max_gradient.lateral * front_gradient_depth > gradient_lateral * max_gradient.depth {
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
            if !cur_opaque && in_range {
                // see beyond the current section
                next.push(ScanParams {
                    min_gradient,
                    max_gradient,
                    min_inclusive,
                    max_inclusive,
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
        //TODO it should be possbile to encode in min_inclusive and max_inclusive
        // whether an octant can "see ahead", removing the need for Octant::should_see
        self.queue_a.push(ScanParams::octant_base(
            In::initial_visibility(),
            true,
            true,
        ));
        self.queue_b.push(ScanParams::octant_base(
            In::initial_visibility(),
            true,
            true,
        ));

        loop {
            let mut corner_bitmap = DirectionBitmap::empty();
            let mut corner_coord = None;

            println!("\n\n#### DEPTH {}\n", self.queue_a[0].depth);

            for params in self.queue_a.drain(..) {
                println!("  {:#?}", params);
                if let Some(corner) =
                    scan(&octant_a, &mut self.queue_a_swap, params, static_params, f)
                {
                    corner_bitmap |= corner.bitmap;
                    corner_coord = Some(corner.coord);
                }
            }

            for params in self.queue_b.drain(..) {
                /*
                if let Some(corner) =
                    scan(&octant_b, &mut self.queue_b_swap, params, static_params, f)
                {
                    corner_bitmap |= corner.bitmap;
                } */
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

    pub fn for_each<F, In>(&mut self, coord: Coord, input_grid: &In, distance: u32, mut f: F)
    where
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
        /*
        self.observe_octant(TopLeft, LeftTop, &params, &mut f);
        self.observe_octant(TopRight { width }, RightTop { width }, &params, &mut f);
        self.observe_octant(
            BottomLeft { height },
            LeftBottom { height },
            &params,
            &mut f,
        ); */
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
