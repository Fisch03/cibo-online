use crate::client::{Client, MoveDirection};
use alloc::{vec, vec::Vec};
#[allow(unused_imports)]
use micromath::F32Ext;
use monos_gfx::{image::SliceReader, Dimension, Image};

macro_rules! include_ppm {
    ($file:expr) => {
        Image::from_ppm(&SliceReader::new(include_bytes!(concat!(
            "../../../assets/",
            $file
        ))))
        .expect(concat!("Failed to load ", $file))
    };
}

macro_rules! include_pbm {
    ($file:expr) => {
        Image::from_pbm(&SliceReader::new(include_bytes!(concat!(
            "../../../assets/",
            $file
        ))))
        .expect(concat!("Failed to load ", $file))
    };
    () => {};
}

#[derive(Debug, Clone)]
pub struct Assets {
    pub cibo: CiboAssets,
    pub tiles: [TileAssets; 2],

    pub message_board: Image,
    pub message_board_bg: Image,

    pub easel: Image,
    pub palette: Image,
    pub palette_mask: Image,
    pub smudge_brush: Image,
    pub paint_tube: Image,
    pub paint_tube_mask: Image,
    pub spatula: Image,

    pub beach_ball: BeachBallAssets,
}

#[derive(Debug, Clone)]
pub struct TileAssets {
    tiles: Vec<(usize, Image)>,
}

#[derive(Debug, Clone)]
pub struct CiboAssets {
    front: CiboImage,
    back: CiboImage,
    left: CiboImage,
    right: CiboImage,
}

#[derive(Debug, Clone)]
pub struct BeachBallAssets {
    image_0: Image,
    image_45: Image,
    image_90: Image,
    image_135: Image,
    image_180: Image,
    image_225: Image,
    image_270: Image,
    image_315: Image,
}

#[derive(Debug, Clone)]
struct CiboImage {
    stand: Image,
    walk: [Image; 2],
}

impl Assets {
    pub fn new() -> Self {
        Self {
            cibo: CiboAssets::new(),
            tiles: [
                TileAssets::new(vec![
                    (12, include_ppm!("tile_plain.ppm")),
                    (3, include_ppm!("tile_grass.ppm")),
                    (1, include_ppm!("tile_flowers.ppm")),
                    (1, include_ppm!("tile_rocks.ppm")),
                ]),
                TileAssets::new(vec![
                    (80, include_ppm!("tile_sand.ppm")),
                    (8, include_ppm!("tile_sand_rocky1.ppm")),
                    (8, include_ppm!("tile_sand_rocky2.ppm")),
                    (1, include_ppm!("tile_seashell.ppm")),
                    (1, include_ppm!("tile_seastar.ppm")),
                ]),
            ],

            message_board: include_ppm!("msgboard.ppm"),
            message_board_bg: include_ppm!("msgboard_bg.ppm"),

            easel: include_ppm!("easel.ppm"),
            palette: include_ppm!("palette.ppm"),
            palette_mask: include_pbm!("palette_mask.pbm"),
            smudge_brush: include_ppm!("smudge_brush.ppm"),
            paint_tube: include_ppm!("paint_tube.ppm"),
            paint_tube_mask: include_pbm!("paint_tube_mask.pbm"),
            spatula: include_ppm!("spatula.ppm"),

            beach_ball: BeachBallAssets::new(),
        }
    }
}

impl TileAssets {
    fn new(mut tiles: Vec<(usize, Image)>) -> Self {
        assert!(!tiles.is_empty());

        let mut weighted_tiles = Vec::with_capacity(tiles.len());
        let mut threshold = 0;
        for (weight, tile) in tiles.drain(..) {
            threshold += weight;
            weighted_tiles.push((threshold, tile));
        }

        Self {
            tiles: weighted_tiles,
        }
    }

    pub fn from_coords(&self, x: i64, y: i64) -> &Image {
        // cheap hash function for random-ish tile selection
        let h = x.wrapping_mul(374761393) + y.wrapping_mul(668265263);
        let h = (h ^ (h >> 13)).wrapping_mul(1274126177);
        let h = h ^ (h >> 16);
        let h = h as usize % self.tiles.last().unwrap().0;

        for (threshold, tile) in self.tiles.iter() {
            if h < *threshold {
                return tile;
            }
        }

        unreachable!()
    }
}

impl CiboAssets {
    fn new() -> Self {
        macro_rules! include_cibo {
            ($name:expr) => {
                CiboImage {
                    stand: include_ppm!(concat!($name, "_stand.ppm")),
                    walk: [
                        include_ppm!(concat!($name, "_walk1.ppm")),
                        include_ppm!(concat!($name, "_walk2.ppm")),
                    ],
                }
            };
        }

        Self {
            front: include_cibo!("cibo_front"),
            back: include_cibo!("cibo_back"),
            left: include_cibo!("cibo_left"),
            right: include_cibo!("cibo_right"),
        }
    }

    fn get_image(&self, direction: MoveDirection) -> &CiboImage {
        match direction {
            MoveDirection::Up => &self.back,
            MoveDirection::Down => &self.front,
            MoveDirection::Left => &self.left,
            MoveDirection::Right => &self.right,
            MoveDirection::None => &self.front,
        }
    }

    pub fn get_client_image(&self, client: &Client, walk_frame: usize) -> &Image {
        let walk_frame = walk_frame % 2;
        if client.movement != MoveDirection::None {
            &self.get_image(client.movement).walk[walk_frame]
        } else {
            &self.get_image(client.look_direction).stand
        }
    }
}

impl BeachBallAssets {
    fn new() -> Self {
        Self {
            image_0: include_ppm!("beach_ball_0.ppm"),
            image_45: include_ppm!("beach_ball_45.ppm"),
            image_90: include_ppm!("beach_ball_90.ppm"),
            image_135: include_ppm!("beach_ball_135.ppm"),
            image_180: include_ppm!("beach_ball_180.ppm"),
            image_225: include_ppm!("beach_ball_225.ppm"),
            image_270: include_ppm!("beach_ball_270.ppm"),
            image_315: include_ppm!("beach_ball_315.ppm"),
        }
    }

    pub fn dimensions(&self) -> Dimension {
        self.image_0.dimensions()
    }

    pub fn get_image(&self, angle: f32) -> &Image {
        const ANGLE_STEP: f32 = 45.0;
        let angle = angle.rem_euclid(360.0);
        let angle = (angle / ANGLE_STEP).round() * ANGLE_STEP;

        match angle as u32 {
            0 => &self.image_0,
            45 => &self.image_45,
            90 => &self.image_90,
            135 => &self.image_135,
            180 => &self.image_180,
            225 => &self.image_225,
            270 => &self.image_270,
            315 => &self.image_315,
            360 => &self.image_0,
            _ => unreachable!(),
        }
    }
}
