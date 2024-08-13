use crate::client::{Client, MoveDirection};
use monos_gfx::{image::SliceReader, Image};

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
    pub tiles: [TileAssets; 1],
    pub message_board: Image,
    pub message_board_bg: Image,
    pub easel: Image,
    pub palette: Image,
    pub palette_mask: Image,
    pub smudge_brush: Image,
    pub paint_tube: Image,
    pub paint_tube_mask: Image,
    pub spatula: Image,
}

#[derive(Debug, Clone)]
pub struct TileAssets {
    pub main_tile: Image,
    pub secondary_tile: Image,
    pub alt_tile1: Image,
    pub alt_tile2: Image,
}

#[derive(Debug, Clone)]
pub struct CiboAssets {
    front: CiboImage,
    back: CiboImage,
    left: CiboImage,
    right: CiboImage,
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
            tiles: [TileAssets::new(
                include_ppm!("tile_plain.ppm"),
                include_ppm!("tile_grass.ppm"),
                include_ppm!("tile_flowers.ppm"),
                include_ppm!("tile_rocks.ppm"),
            )],
            message_board: include_ppm!("msgboard.ppm"),
            message_board_bg: include_ppm!("msgboard_bg.ppm"),
            easel: include_ppm!("easel.ppm"),
            palette: include_ppm!("palette.ppm"),
            palette_mask: include_pbm!("palette_mask.pbm"),
            smudge_brush: include_ppm!("smudge_brush.ppm"),
            paint_tube: include_ppm!("paint_tube.ppm"),
            paint_tube_mask: include_pbm!("paint_tube_mask.pbm"),
            spatula: include_ppm!("spatula.ppm"),
        }
    }
}

impl TileAssets {
    fn new(main_tile: Image, secondary_tile: Image, alt_tile1: Image, alt_tile2: Image) -> Self {
        Self {
            main_tile,
            secondary_tile,
            alt_tile1,
            alt_tile2,
        }
    }

    pub fn from_coords(&self, x: i64, y: i64) -> &Image {
        // cheap hash function for random-ish tile selection
        let h = x.wrapping_mul(374761393) + y.wrapping_mul(668265263);
        let h = (h ^ (h >> 13)).wrapping_mul(1274126177);
        let h = h ^ (h >> 16);
        match h % 10 {
            0..7 => &self.main_tile,
            7..8 => &self.secondary_tile,
            8 => &self.alt_tile1,
            9 => &self.alt_tile2,
            _ => unreachable!(),
        }
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
