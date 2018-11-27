use super::*;
use coord_2d::*;
use direction::*;

struct Grid<T> {
    size: Size,
    cells: Vec<T>,
}

impl<T> Grid<T> {
    fn new_fn<F: FnMut(Coord) -> T>(size: Size, mut f: F) -> Self {
        let mut cells = Vec::with_capacity(size.count());
        for i in 0..size.height() {
            for j in 0..size.width() {
                cells.push(f(Coord::new(j as i32, i as i32)));
            }
        }
        Self { size, cells }
    }
    fn index(&self, coord: Coord) -> Option<usize> {
        if coord.is_valid(self.size) {
            Some((coord.y as u32 * self.size.width() + coord.x as u32) as usize)
        } else {
            None
        }
    }
    fn get(&self, coord: Coord) -> Option<&T> {
        self.index(coord).map(|index| &self.cells[index])
    }
    fn get_mut(&mut self, coord: Coord) -> Option<&mut T> {
        self.index(coord).map(move |index| &mut self.cells[index])
    }
}

type TestInputGrid = Grid<u8>;
type TestOutputGrid = Grid<Option<DirectionBitmap>>;

impl OutputGrid for TestOutputGrid {
    fn see(&mut self, coord: Coord, d: DirectionBitmap, _time: u64) {
        if let Some(v) = self.get_mut(coord) {
            if v.is_some() {
                //panic!("already have value at {:?}", coord);
            }
            *v = Some(d);
        }
    }
}

impl InputGrid for TestInputGrid {
    type Opacity = u8;
    type Visibility = u8;
    fn size(&self) -> Size {
        self.size
    }
    fn get_opacity(&self, coord: Coord) -> Option<Self::Opacity> {
        self.get(coord).cloned()
    }
    fn initial_visibility() -> Self::Visibility {
        255
    }
}

fn input_from_strs(strs: &[&str]) -> (TestInputGrid, Coord) {
    let size = Size::new(strs[0].len() as u32, strs.len() as u32);
    let mut grid = Grid::new_fn(size, |_| 0);
    let mut eye = None;
    for (i, row) in strs.iter().enumerate() {
        for (j, ch) in row.chars().enumerate() {
            let coord = Coord::new(j as i32, i as i32);
            let cell = match ch {
                '@' => {
                    eye = Some(coord);
                    0
                }
                '.' => 0,
                '#' => 255,
                '&' => 128,
                _ => panic!("unknown char"),
            };
            *grid.get_mut(coord).expect("out of bounds") = cell;
        }
    }
    (grid, eye.expect("no eye"))
}

fn output_to_strings(eye: Coord, grid: &TestOutputGrid) -> Vec<String> {
    let mut strings = Vec::new();
    for i in 0..grid.size.height() {
        let mut string = String::new();
        for j in 0..grid.size.width() {
            let coord = Coord::new(j as i32, i as i32);
            let ch = if coord == eye {
                '@'
            } else if let Some(ref directions) = grid.get(coord).unwrap() {
                use self::Direction::*;
                let directions = *directions;
                if directions == DirectionBitmap::all() {
                    ','
                } else if directions == North.bitmap() {
                    '▀'
                } else if directions == East.bitmap() {
                    '▐'
                } else if directions == South.bitmap() {
                    '▄'
                } else if directions == West.bitmap() {
                    '▌'
                } else if directions == NorthEast.bitmap() {
                    '▝'
                } else if directions == NorthWest.bitmap() {
                    '▘'
                } else if directions == SouthWest.bitmap() {
                    '▖'
                } else if directions == SouthEast.bitmap() {
                    '▗'
                } else if directions == North.bitmap() | East.bitmap() {
                    '▜'
                } else if directions == South.bitmap() | East.bitmap() {
                    '▟'
                } else if directions == South.bitmap() | West.bitmap() {
                    '▙'
                } else if directions == North.bitmap() | West.bitmap() {
                    '▛'
                } else {
                    eprintln!("unknown {:b}", directions.raw);
                    '?'
                }
            } else {
                '%'
            };
            string.push(ch);
        }
        strings.push(string);
    }
    strings
}

fn check_output(eye: Coord, output: &TestOutputGrid, expected_output_strings: &[&str]) {
    let output_strings = output_to_strings(eye, output);
    if output_strings != expected_output_strings {
        panic!("Unexpected output:\n{:#?}", output_strings);
    }
}

fn check_scenario(input_strs: &[&str], expected_output: &[&str]) {
    let (input, eye) = input_from_strs(input_strs);
    let mut output = Grid::new_fn(input.size, |_| None);
    let mut ctx: ShadowcastContext<u8> = ShadowcastContext::new();
    ctx.observe(eye, &input, 100, 1, &mut output);
    check_output(eye, &output, expected_output);
}

#[test]
fn single() {
    check_scenario(&["@"], &["@"]);
}

#[test]
fn empty() {
    check_scenario(
        &[
            "...........",
            "...........",
            "...........",
            "...........",
            ".....@.....",
            "...........",
            "...........",
            "...........",
            "...........",
        ],
        &[
            ",,,,,,,,,,,",
            ",,,,,,,,,,,",
            ",,,,,,,,,,,",
            ",,,,,,,,,,,",
            ",,,,,@,,,,,",
            ",,,,,,,,,,,",
            ",,,,,,,,,,,",
            ",,,,,,,,,,,",
            ",,,,,,,,,,,",
        ],
    );
}

#[test]
fn full() {
    check_scenario(
        &[
            "###########",
            "###########",
            "###########",
            "###########",
            "#####@#####",
            "###########",
            "###########",
            "###########",
            "###########",
        ],
        &[
            "%%%%%%%%%%%",
            "%%%%%%%%%%%",
            "%%%%%%%%%%%",
            "%%%%▗▄▖%%%%",
            "%%%%▐@▌%%%%",
            "%%%%▝▀▘%%%%",
            "%%%%%%%%%%%",
            "%%%%%%%%%%%",
            "%%%%%%%%%%%",
        ],
    );
}

#[test]
fn corners() {
    check_scenario(
        &[
            "...............",
            ".#############.",
            ".#...........#.",
            ".#...........#.",
            ".#.......#...#.",
            ".#...........#.",
            ".#..#........#.",
            ".#.....@.....#.",
            ".#...........#.",
            ".#...........#.",
            ".#.......#...#.",
            ".#....#......#.",
            ".#...........#.",
            ".#...........#.",
            ".#...........#.",
            ".#############.",
            "...............",
        ],
        &[
            "%%%%%%%%%%%%%%%",
            "%▗▄▄▄▄▄▄▄▄▖%%▖%",
            "%▐,,,,,,,,%%,▌%",
            "%▐,,,,,,,,%,,▌%",
            "%▐,,,,,,,▙,,,▌%",
            "%%%,,,,,,,,,,▌%",
            "%▐,,▟,,,,,,,,▌%",
            "%▐,,,,,@,,,,,▌%",
            "%▐,,,,,,,,,,,▌%",
            "%▐,,,,,,,,,,,▌%",
            "%▐,,,,,,,▛,,,▌%",
            "%▐,,,,▜,,,%,,▌%",
            "%▐,,,,,,,,%%,▌%",
            "%▐,,,,,,,,,%%▘%",
            "%▐,,,%,,,,,%%%%",
            "%▝▀▀▀%▀▀▀▀▀▘%%%",
            "%%%%%%%%%%%%%%%",
        ],
    );
}

#[test]
fn gaps() {
    check_scenario(
        &[
            "..........#",
            "......#...#",
            "..##..#...#",
            "..........#",
            "...@......#",
            "......#...#",
            "##....#...#",
            "..........#",
            "####..##..#",
        ],
        &[
            "%%%%,,,%%%%",
            ",%%%,,▌%%,▌",
            ",,▄▄,,▙,,,▌",
            ",,,,,,,,,,▌",
            ",,,@,,,,,,▌",
            ",,,,,,▛,,,▌",
            "▀▜,,,,▌%%%%",
            "%,,,,,,%%%%",
            "▝▀▀▀,,▛▘%%%",
        ],
    );
}

#[test]
fn transparency() {
    check_scenario(
        &[
            "@....................",
            ".....................",
            ".....................",
            ".....................",
            ".....................",
            ".....................",
            ".....................",
            ".....................",
            ".....................",
            "........&............",
            "........&............",
            "........&............",
            "........&............",
            "........&............",
            "........&............",
            "........&............",
            "........&............",
            "........&............",
            "........&............",
            "........&............",
            "........&............",
            "........&............",
            "........&............",
            "........&............",
            "........&............",
        ],
        &[
            ",,,,,,,,,,,",
            ",,,,,,,,,,,",
            ",,,,,,,,,,,",
            ",,,,,,,,,,,",
            ",,,,,@,,,,,",
            ",,,,,,,,,,,",
            ",,,,,,,,,,,",
            ",,,,,,,,,,,",
            ",,,,,,,,,,,",
        ],
    );
}
