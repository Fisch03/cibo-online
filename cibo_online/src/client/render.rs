use super::{chat_widget::ChatWidget, ClientGameState, ClientMessage};
use crate::game_state::{Client, ClientId, MoveDirection};
use alloc::vec::Vec;

use monos_gfx::{
    image::SliceReader,
    input::Input,
    text::{font, Origin},
    types::*,
    ui::{widgets, Direction, MarginMode, TextWrap, UIContext, UIFrame},
    Framebuffer, Image,
};

const CAMERA_EDGE: i64 = 75;
const WALK_FRAME_DURATION: usize = 250;

macro_rules! include_ppm {
    ($file:expr) => {
        Image::from_ppm(&SliceReader::new(include_bytes!(concat!(
            "../../../assets/",
            $file
        ))))
        .expect(concat!("Failed to load ", $file))
    };
}

#[derive(Debug, Clone)]
pub struct RenderState {
    camera: Position,
    client_uis: Vec<(ClientId, UIFrame)>,
    chat_log: UIFrame,

    assets: Assets,
}

impl RenderState {
    pub fn cleanup_client(&mut self, client_id: &ClientId) {
        self.client_uis.retain(|(id, _)| id != client_id);
    }
}

impl Default for RenderState {
    fn default() -> Self {
        let camera = Position::new(0, 0);

        Self {
            camera,
            assets: Assets::new(),
            client_uis: Vec::new(),
            chat_log: UIFrame::new(Direction::BottomToTop),
        }
    }
}

#[derive(Debug, Clone)]
struct Assets {
    cibo: CiboAssets,
    floor_tiles: [Image; 4],
}

#[derive(Debug, Clone)]
struct CiboAssets {
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
    fn new() -> Self {
        let floor_tiles = [
            include_ppm!("tile_plain.ppm"),
            include_ppm!("tile_grass.ppm"),
            include_ppm!("tile_flowers.ppm"),
            include_ppm!("tile_rocks.ppm"),
        ];

        Self {
            cibo: CiboAssets::new(),
            floor_tiles,
        }
    }

    fn tile_from_coords(&self, x: i64, y: i64) -> &Image {
        // cheap hash function for random-ish tile selection
        let h = x.wrapping_mul(374761393) + y.wrapping_mul(668265263);
        let h = (h ^ (h >> 13)) * 1274126177;
        let h = h ^ (h >> 16);
        match h % 10 {
            0..7 => &self.floor_tiles[0],
            7..8 => &self.floor_tiles[1],
            8 => &self.floor_tiles[2],
            9 => &self.floor_tiles[3],
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

    fn get_client_image(&self, client: &Client, walk_frame: usize) -> &Image {
        let walk_frame = walk_frame % 2;
        if client.movement != MoveDirection::None {
            &self.get_image(client.movement).walk[walk_frame]
        } else {
            &self.get_image(client.look_direction).stand
        }
    }
}

impl ClientGameState {
    pub(super) fn render(
        &mut self,
        framebuffer: &mut Framebuffer,
        input: &mut Input,
        send_msg: &mut dyn FnMut(ClientMessage),
    ) {
        let walk_frame = self.local.time_ms as usize / WALK_FRAME_DURATION;
        let render_state = &mut self.local.render_state;

        // move camera to follow client
        let mut client_screen_position = self.client.position - render_state.camera;
        if client_screen_position.x < CAMERA_EDGE - 32 {
            render_state.camera.x = self.client.position.x - CAMERA_EDGE + 32;
            client_screen_position.x = CAMERA_EDGE - 32;
        } else if client_screen_position.x > framebuffer.dimensions().width as i64 - CAMERA_EDGE {
            render_state.camera.x =
                self.client.position.x - framebuffer.dimensions().width as i64 + CAMERA_EDGE;
            client_screen_position.x = framebuffer.dimensions().width as i64 - CAMERA_EDGE;
        }

        if client_screen_position.y < CAMERA_EDGE - 32 {
            render_state.camera.y = self.client.position.y - CAMERA_EDGE + 32;
            client_screen_position.y = CAMERA_EDGE - 32;
        } else if client_screen_position.y > framebuffer.dimensions().height as i64 - CAMERA_EDGE {
            render_state.camera.y =
                self.client.position.y - framebuffer.dimensions().height as i64 + CAMERA_EDGE;
            client_screen_position.y = framebuffer.dimensions().height as i64 - CAMERA_EDGE;
        }

        // draw floor
        let start_tile = render_state.camera / 16;
        let fb_tile_size = framebuffer.dimensions() / 16;

        for x in start_tile.x - 1..start_tile.x + fb_tile_size.width as i64 + 1 {
            for y in start_tile.y - 1..start_tile.y + fb_tile_size.height as i64 + 1 {
                let position = Position::new(x * 16, y * 16) - render_state.camera;
                framebuffer.draw_img(render_state.assets.tile_from_coords(x, y), &position);
            }
        }

        macro_rules! draw_client {
            ($client: expr) => {
                draw_client!($client, |_: &mut UIContext| {});
            };
            ($client: expr,  $additional_ui:expr) => {
                let screen_position = $client.position - render_state.camera;

                let ui = if let Some(ui) = render_state
                    .client_uis
                    .iter_mut()
                    .find(|(id, _)| id == &$client.id())
                    .map(|(_, ui)| ui)
                {
                    ui
                } else {
                    let ui = UIFrame::new(Direction::BottomToTop);
                    render_state.client_uis.push(($client.id(), ui));
                    &mut render_state.client_uis.last_mut().unwrap().1
                };

                // draw client
                let client_image = render_state
                    .assets
                    .cibo
                    .get_client_image(&$client, walk_frame);
                framebuffer.draw_img(client_image, &screen_position);

                // draw client chat
                let client_ui_rect = Rect::new(
                    Position::new(screen_position.x - 30, -i64::MAX),
                    Position::new(screen_position.x + 30 + 32, screen_position.y + 45),
                );

                ui.draw_frame(framebuffer, client_ui_rect, input, |ui| {
                    ui.margin(MarginMode::Grow);

                    ui.label::<font::Glean>(&$client.name());

                    ui.alloc_space(Dimension::new(0, 26));

                    $additional_ui(ui);

                    for chat in self
                        .local
                        .other_chat
                        .iter()
                        .rev()
                        .filter(|c| c.client_id == $client.id())
                        .take(3)
                    {
                        ui.add(ChatWidget::new(&chat.message));
                    }
                });
            };
        }

        // draw players behind client
        for client in self
            .game_state
            .clients
            .iter()
            .filter(|c| c.position.y <= self.client.position.y)
        {
            draw_client!(client);
        }

        draw_client!(self.client, |ui: &mut UIContext| {
            if let Some(chat) = &mut self.local.own_chat {
                let textbox = widgets::Textbox::<font::Glean>::new(chat)
                    .wrap(TextWrap::Enabled { hyphenate: false });
                if ui.add(textbox).submitted {
                    if !chat.is_empty() {
                        send_msg(ClientMessage::Chat(chat.clone()));
                    }

                    self.local.own_chat = None;
                }
            }
        });

        // draw players in front of client
        for client in self
            .game_state
            .clients
            .iter()
            .filter(|c| c.position.y > self.client.position.y)
        {
            draw_client!(client);
        }

        let chat_log_rect = Rect::new(
            Position::new(0, framebuffer.dimensions().height as i64 - 100),
            Position::new(100, framebuffer.dimensions().height as i64),
        );
        render_state
            .chat_log
            .draw_frame(framebuffer, chat_log_rect, input, |ui| {
                ui.add(
                    widgets::ScrollableLabel::<font::Glean, _>::new_iter(
                        self.local.chat_log.iter().map(|chat| chat.as_str()),
                        Origin::Bottom,
                    )
                    .wrap(TextWrap::Enabled { hyphenate: false })
                    .scroll_y(100),
                );
            });
    }
}
