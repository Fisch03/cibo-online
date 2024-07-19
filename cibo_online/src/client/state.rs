use crate::{server::ServerMessage, RenderContext, Renderable, WorldLocalState, WorldState};

use super::{Client, ClientAction, ClientId, ClientMessage, MoveDirection};
use alloc::{boxed::Box, collections::VecDeque, format, string::String};
use monos_gfx::{
    input::{Input, Key, KeyState, RawKey},
    text::{font, Origin, TextWrap},
    ui::{widgets, Direction, MarginMode, UIFrame},
    Edge, Framebuffer, Position, Rect,
};
use serde::{Deserialize, Serialize};

const CAMERA_EDGE_X: i64 = 100;
const CAMERA_EDGE_Y: i64 = 50;
const INTERACTABLE_FOCUS_SPEED: i64 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientGameState {
    pub(crate) own_id: ClientId,
    pub(crate) world: WorldState,

    #[serde(skip)]
    pub(crate) local: Option<Box<ClientLocalState>>, // boxed to avoid allocating space on the server
}

// safety: the ClientLocalState is only instantiated by the client which is single-threaded.
unsafe impl Send for ClientLocalState {}
//...the cloning also only happens on the server so this is fine too!
impl Clone for ClientLocalState {
    fn clone(&self) -> Self {
        panic!("ClientLocalState cannot be cloned")
    }
}

#[derive(Debug)]
pub struct ClientLocalState {
    time_ms: u64,
    last_tick: u64,
    last_message: u64,

    render: RenderState,

    world: WorldLocalState,
}

#[derive(Debug, Clone)]
struct RenderState {
    stream_mode: bool,

    camera: Position,

    chat_log: VecDeque<String>,
    chat_log_ui: UIFrame,

    coordinate_ui: UIFrame,
    player_list_ui: Option<UIFrame>,
}

impl Default for RenderState {
    fn default() -> Self {
        RenderState {
            stream_mode: false,
            camera: Position::new(0, 0),
            chat_log: VecDeque::new(),
            chat_log_ui: UIFrame::new(Direction::BottomToTop),
            coordinate_ui: UIFrame::new_stateless(Direction::RightToLeft),
            player_list_ui: None,
        }
    }
}

impl ClientLocalState {
    fn new(own_id: ClientId) -> Self {
        ClientLocalState {
            time_ms: 0,
            last_tick: 0,
            last_message: 0,

            render: Default::default(),

            world: WorldLocalState::new(own_id),
        }
    }
}

impl ClientGameState {
    pub(crate) fn new(client_id: ClientId, mut world: WorldState) -> Self {
        let own_client = world.clients.iter().position(|c| c.id() == client_id);
        world.clients.swap(0, own_client.unwrap());

        ClientGameState {
            own_id: client_id,
            world,

            local: None,
        }
    }

    #[inline(always)]
    pub fn client(&self) -> &Client {
        self.world.clients.first().unwrap()
    }

    #[inline(always)]
    fn client_mut(&mut self) -> &mut Client {
        self.world.clients.first_mut().unwrap()
    }

    fn prepare_local(&mut self) {
        self.local
            .get_or_insert_with(|| Box::new(ClientLocalState::new(self.own_id)));
    }

    #[inline(always)]
    fn local(&self) -> &ClientLocalState {
        self.local.as_ref().unwrap()
    }

    #[inline(always)]
    fn local_mut(&mut self) -> &mut ClientLocalState {
        self.local.as_mut().unwrap()
    }

    pub fn update(
        &mut self,
        delta_ms: u64,
        framebuffer: &mut Framebuffer,
        input: &mut Input,
        send_msg: &mut dyn FnMut(ClientMessage),
    ) {
        self.prepare_local();

        self.local_mut().time_ms += delta_ms;

        let mut client_action = ClientAction::new();

        let tick_amt = (self.local().time_ms - self.local().last_tick) / crate::SERVER_TICK_RATE;
        self.local_mut().last_tick += tick_amt * crate::SERVER_TICK_RATE;

        let mut direction = match self.client().movement {
            MoveDirection::None => None,
            direction => Some(direction),
        };

        if self.local().world.own_local.borrow().chat_input.is_none() {
            for input in &input.keyboard {
                let button_direction = match input.key {
                    Key::RawKey(RawKey::ArrowUp) | Key::Unicode('w') => Some(MoveDirection::Up),
                    Key::RawKey(RawKey::ArrowDown) | Key::Unicode('s') => Some(MoveDirection::Down),
                    Key::RawKey(RawKey::ArrowLeft) | Key::Unicode('a') => Some(MoveDirection::Left),
                    Key::RawKey(RawKey::ArrowRight) | Key::Unicode('d') => {
                        Some(MoveDirection::Right)
                    }

                    Key::RawKey(RawKey::Return) | Key::Unicode('t')
                        if input.state == KeyState::Down =>
                    {
                        self.local().world.own_local.borrow_mut().chat_input = Some(String::new());
                        client_action.typing(true);
                        Some(MoveDirection::None)
                    }
                    _ => None,
                };

                if let Some(button_direction) = button_direction {
                    match input.state {
                        KeyState::Down => {
                            direction = Some(button_direction);
                        }
                        KeyState::Up => {
                            if direction == Some(button_direction) {
                                direction = Some(MoveDirection::None);
                            }
                        }
                        _ => {}
                    }
                }
            }

            // remove the return key from the input queue to avoid instantly closing the chat again
            if self
                .local()
                .world
                .own_local
                .borrow_mut()
                .chat_input
                .is_some()
            {
                input
                    .keyboard
                    .retain(|k| k.key != Key::RawKey(RawKey::Return));
            }
        } else {
            for input in input.keyboard.iter() {
                match input.key {
                    Key::RawKey(RawKey::Escape) if input.state == KeyState::Down => {
                        self.local().world.own_local.borrow_mut().chat_input = None;
                        client_action.typing(false);
                    }
                    _ => {}
                }
            }
        }

        let checked_pos = match direction {
            Some(MoveDirection::None) => {
                client_action.movement(self.client().position, MoveDirection::None);
                None
            }
            Some(direction) => {
                let mut position = self.client().position;
                match direction {
                    MoveDirection::Up => position.y -= 1 * tick_amt as i64,
                    MoveDirection::Down => position.y += 1 * tick_amt as i64,
                    MoveDirection::Left => position.x -= 1 * tick_amt as i64,
                    MoveDirection::Right => position.x += 1 * tick_amt as i64,
                    MoveDirection::None => unreachable!(),
                }

                Some(position)
            }
            _ => None,
        };
        let own_hitbox = checked_pos.map(|pos| {
            Rect::new(
                Position::new(pos.x + 2, pos.y + 12),
                Position::new(pos.x + 30, pos.y + 32),
            )
        });

        let mut camera = self.local().render.camera;
        let camera_rect = Rect::new(
            Position::new(camera.x, camera.y),
            Position::new(
                camera.x + framebuffer.dimensions().width as i64,
                camera.y + framebuffer.dimensions().height as i64,
            ),
        );

        let mut collision = false;
        for object in &self.local().world.objects {
            // collide with objects
            if let Some(object_hitbox) = object.hitbox() {
                match own_hitbox {
                    Some(ref own_hitbox) if !collision => {
                        if object_hitbox.intersects(own_hitbox) {
                            collision = true;
                        }
                    }
                    _ => {}
                }
            }

            // if an interactable object only partially visible, nudge the camera to show it
            if object.interacts_with(self.client().position) {
                match object.bounds().intersects_edge(&camera_rect) {
                    Some(Edge::Top) => camera.y -= 1,
                    Some(Edge::Bottom) => camera.y += 1,
                    Some(Edge::Left) => camera.x -= 1,
                    Some(Edge::Right) => camera.x += 1,
                    None => {}
                }
            }
        }

        self.local_mut().render.camera = camera;
        if let Some(direction) = direction {
            match checked_pos {
                Some(position) if !collision => client_action.movement(position, direction),
                _ => client_action.look(direction),
            }
        }

        let forced_update =
            self.local().time_ms - self.local().last_message > crate::SERVER_TICK_RATE * 15;
        let has_action = client_action.any();
        if has_action || forced_update {
            if forced_update && !has_action {
                client_action.movement(self.client().position, self.client().movement);
                client_action.typing(
                    self.local()
                        .world
                        .own_local
                        .borrow_mut()
                        .chat_input
                        .is_some(),
                );
                self.local_mut().last_message = self.local().time_ms;
            }

            self.client_mut().apply_action(&client_action);
            send_msg(ClientMessage::Action(client_action))
        }

        self.render(framebuffer, input, send_msg);

        // for object in self.local().world.objects.iter() {
        //     if let Some(hitbox) = object.hitbox() {
        //         let camera = self.local().render.camera;
        //         let hitbox = hitbox.translate(Position::new(-camera.x, -camera.y));
        //         framebuffer.draw_box(&hitbox, &monos_gfx::Color::new(255, 0, 0))
        //     }
        //
        //     let bounds = object
        //         .bounds()
        //         .translate(Position::new(-camera.x, -camera.y));
        //     framebuffer.draw_box(&bounds, &monos_gfx::Color::new(0, 255, 0))
        // }

        input.clear();
    }

    pub fn handle_message(&mut self, msg: ServerMessage) {
        match msg {
            ServerMessage::NewClient(client) => {
                let client_id = client.id();
                self.world.clients.push(client);
                self.local_mut()
                    .world
                    .clients
                    .push((client_id, Default::default()));
            }
            ServerMessage::ClientLeft(client_id) => {
                if client_id != self.client().id() {
                    self.world.clients.retain(|c| c.id() != client_id);
                    self.local_mut()
                        .world
                        .clients
                        .retain(|(id, _)| *id != client_id);
                }
            }
            ServerMessage::FullState(_) => {
                panic!(
                    "unexpected FullState message. should be handled by the client implementation"
                );
            }
            ServerMessage::UpdateState(updates) => {
                for (id, action) in updates {
                    if id == self.client().id() {
                        continue;
                    }
                    if let Some(client) = self.world.clients.iter_mut().find(|c| c.id() == id) {
                        client.apply_action(&action);
                    }
                }
            }
            ServerMessage::Chat(client_id, message) => {
                let client = self.world.clients.iter_mut().find(|c| c.id() == client_id);

                let client_name;
                if let Some(client) = client {
                    client.typing = false;
                    client_name = client.name()
                } else {
                    if client_id == self.client().id() {
                        client_name = "You";
                    } else {
                        client_name = "Unknown";
                    }
                };

                let log_line = format!("<{}> {}", client_name, message);
                let local = self.local_mut();

                let render_state = &mut local.render;
                render_state.chat_log.push_back(log_line);
                if render_state.chat_log.len() > 256 {
                    render_state.chat_log.pop_front();
                }

                local
                    .world
                    .add_chat(client_id, message, local.time_ms + 5000);
            }
        }
    }

    pub fn render(
        &mut self,
        framebuffer: &mut Framebuffer,
        input: &mut Input,
        send_msg: &mut dyn FnMut(ClientMessage),
    ) {
        input.keyboard.iter().for_each(|input| match input.key {
            Key::RawKey(RawKey::Tab) => {
                self.local_mut().render.player_list_ui = if input.state == KeyState::Down {
                    Some(UIFrame::new_stateless(Direction::TopToBottom))
                } else {
                    None
                }
            }
            Key::RawKey(RawKey::F1) if input.state == KeyState::Down => {
                self.local_mut().render.stream_mode = !self.local().render.stream_mode;
            }
            _ => {}
        });

        // move camera to follow client
        let mut camera = self.local().render.camera;
        let mut client_screen_position = self.client().position - camera;
        if client_screen_position.x < CAMERA_EDGE_X - 32 {
            camera.x = self.client().position.x - CAMERA_EDGE_X + 32;
            client_screen_position.x = CAMERA_EDGE_X - 32;
        } else if client_screen_position.x > framebuffer.dimensions().width as i64 - CAMERA_EDGE_X {
            camera.x =
                self.client().position.x - framebuffer.dimensions().width as i64 + CAMERA_EDGE_X;
            client_screen_position.x = framebuffer.dimensions().width as i64 - CAMERA_EDGE_X;
        }

        if client_screen_position.y < CAMERA_EDGE_Y - 32 {
            camera.y = self.client().position.y - CAMERA_EDGE_Y + 32;
            client_screen_position.y = CAMERA_EDGE_Y - 32;
        } else if client_screen_position.y > framebuffer.dimensions().height as i64 - CAMERA_EDGE_Y
        {
            camera.y =
                self.client().position.y - framebuffer.dimensions().height as i64 + CAMERA_EDGE_Y;
            client_screen_position.y = framebuffer.dimensions().height as i64 - CAMERA_EDGE_Y;
        }
        self.local_mut().render.camera = camera;

        {
            let player_pos = self.client().position;
            let local = self
                .local
                .get_or_insert_with(|| Box::new(ClientLocalState::new(self.own_id)));

            let mut ctx = RenderContext {
                fb: framebuffer,
                time_ms: local.time_ms,
                stream_mode: local.render.stream_mode,
                player_pos,
                input,
                send_msg,
            };

            self.world.render(&mut local.world, camera, &mut ctx);
        }

        // dont draw ui if in stream mode
        if self.local().render.stream_mode {
            return;
        }

        // draw chat log
        let chat_log_rect = Rect::new(
            Position::new(0, framebuffer.dimensions().height as i64 - 100),
            Position::new(100, framebuffer.dimensions().height as i64),
        );

        {
            let local = self.local_mut();
            local
                .render
                .chat_log_ui
                .draw_frame(framebuffer, chat_log_rect, input, |ui| {
                    ui.add(
                        widgets::ScrollableLabel::<font::Glean, _>::new_iter(
                            local.render.chat_log.iter().map(|chat| chat.as_str()),
                            Origin::Bottom,
                        )
                        .wrap(TextWrap::Enabled { hyphenate: false })
                        .scroll_y(100),
                    );
                });
        }

        // draw coordinate display
        let tile_position = self.client().position / 16;
        let coordinate_rect = Rect::new(
            Position::new(framebuffer.dimensions().width as i64 - 100, 0),
            Position::new(framebuffer.dimensions().width as i64, 100),
        );
        self.local_mut().render.coordinate_ui.draw_frame(
            framebuffer,
            coordinate_rect,
            input,
            |ui| {
                ui.label::<font::Glean>(&format!("X{} / Y{}", tile_position.x, tile_position.y));
            },
        );

        // draw player list
        let local = self
            .local
            .get_or_insert_with(|| Box::new(ClientLocalState::new(self.own_id)));
        if let Some(player_list) = &mut local.render.player_list_ui {
            let player_list_rect = Rect::new(
                Position::new(framebuffer.dimensions().width as i64 / 2 - 100, 10),
                Position::new(
                    framebuffer.dimensions().width as i64 / 2 + 100,
                    framebuffer.dimensions().height as i64 - 10,
                ),
            );
            player_list.draw_frame(framebuffer, player_list_rect, input, |ui| {
                ui.margin(MarginMode::Grow);
                ui.label::<font::Cozette>(&format!(
                    "Players Online: {}",
                    self.world.clients.len() + 1
                ));
                ui.label::<font::Glean>("You");
                for client in self.world.clients.iter().skip(1) {
                    let client_tile_position = client.position / 16;
                    ui.label::<font::Glean>(&format!(
                        "{} | X{} / Y{}",
                        client.name(),
                        client_tile_position.x,
                        client_tile_position.y
                    ));
                }
            });
        }
    }
}
