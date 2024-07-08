mod render;

use crate::{server::ServerMessage, Client, ClientAction, ClientId, GameState};

use alloc::{collections::VecDeque, string::String, vec::Vec};
use monos_gfx::{
    input::{Input, Key, KeyEvent, KeyState, RawKey},
    ui::{self, UIFrame},
    Framebuffer,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    Connect { name: String },
    Action(ClientAction),
    Chat(String),
}

impl ClientMessage {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientGameState {
    pub client: Client,
    game_state: GameState,

    #[serde(skip)]
    local: ClientLocalState,
}

#[derive(Debug, Clone)]
pub struct ClientLocalState {
    time_ms: u64,
    last_tick: u64,

    ui_frame: UIFrame,
    own_chat: Option<String>,
    other_chat: VecDeque<ChatMessage>,

    input: Input,
    movement: MoveDirection,
}

impl Default for ClientLocalState {
    fn default() -> Self {
        Self {
            time_ms: 0,
            last_tick: 0,

            ui_frame: UIFrame::new(ui::Direction::TopToBottom),
            own_chat: None,
            other_chat: VecDeque::new(),

            input: Input::default(),
            movement: MoveDirection::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub client_id: ClientId,
    pub message: String,
    pub expiry: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDirection {
    Up,
    Down,
    Left,
    Right,
    None,
}

impl Default for MoveDirection {
    fn default() -> Self {
        MoveDirection::None
    }
}

impl ClientGameState {
    pub(crate) fn new(client: Client, mut game_state: GameState) -> Self {
        game_state.clients.retain(|c| c.id() != client.id());

        ClientGameState {
            client,
            game_state,

            local: ClientLocalState::default(),
        }
    }

    pub fn handle_action(&mut self, action: ClientAction) {
        self.client.apply_action(&action);
    }

    pub fn add_input(&mut self, key_event: KeyEvent) {
        self.local.input.keyboard.push_back(key_event);
    }

    pub fn update(
        &mut self,
        delta_ms: u64,
        framebuffer: &mut Framebuffer,
        send_msg: &mut dyn FnMut(ClientMessage),
    ) {
        self.local.time_ms += delta_ms;

        let mut client_action = ClientAction::new();

        let tick_amt = (self.local.time_ms - self.local.last_tick) / crate::SERVER_TICK_RATE;
        self.local.last_tick += tick_amt * crate::SERVER_TICK_RATE;

        if self.local.own_chat.is_none() {
            while let Some(input) = self.local.input.keyboard.pop_front() {
                let mut direction = None;
                match input.key {
                    Key::RawKey(RawKey::ArrowUp) | Key::Unicode('w') => {
                        direction = Some(MoveDirection::Up)
                    }
                    Key::RawKey(RawKey::ArrowDown) | Key::Unicode('s') => {
                        direction = Some(MoveDirection::Down)
                    }
                    Key::RawKey(RawKey::ArrowLeft) | Key::Unicode('a') => {
                        direction = Some(MoveDirection::Left)
                    }
                    Key::RawKey(RawKey::ArrowRight) | Key::Unicode('d') => {
                        direction = Some(MoveDirection::Right)
                    }

                    Key::RawKey(RawKey::Return) | Key::Unicode('t')
                        if input.state == KeyState::Down =>
                    {
                        self.local.own_chat = Some(String::new());
                    }
                    _ => {}
                }

                if let Some(direction) = direction {
                    match input.state {
                        KeyState::Down => {
                            self.local.movement = direction;
                        }
                        KeyState::Up => {
                            if self.local.movement == direction {
                                self.local.movement = MoveDirection::None;
                            }
                        }
                        _ => {}
                    }
                }
            }

            let mut position = self.client.position();
            match self.local.movement {
                MoveDirection::Up => position.y -= 1 * tick_amt as i64,
                MoveDirection::Down => position.y += 1 * tick_amt as i64,
                MoveDirection::Left => position.x -= 1 * tick_amt as i64,
                MoveDirection::Right => position.x += 1 * tick_amt as i64,
                MoveDirection::None => (),
            }
            client_action.movement(position);
        } else {
            for input in self.local.input.keyboard.iter() {
                match input.key {
                    Key::RawKey(RawKey::Escape) if input.state == KeyState::Down => {
                        self.local.own_chat = None;
                    }
                    _ => {}
                }
            }
        }

        if client_action.any() {
            self.client.apply_action(&client_action);
            send_msg(ClientMessage::Action(client_action))
        }

        self.render(framebuffer, send_msg);

        self.local.input.clear();
    }

    pub fn handle_message(&mut self, msg: ServerMessage) {
        match msg {
            ServerMessage::NewClient(client) => {
                self.game_state.clients.push(client);
                self.game_state.clients.sort_unstable();
            }
            ServerMessage::ClientLeft(client_id) => {
                self.game_state.clients.retain(|c| c.id() != client_id);
            }
            ServerMessage::FullState(_) => {
                panic!(
                    "unexpected FullState message. should be handled by the client implementation"
                );
            }
            ServerMessage::UpdateState(updates) => {
                let mut clients = self.game_state.clients.iter_mut();

                for (id, action) in updates {
                    while let Some(client) = clients.next() {
                        if client.id() != id {
                            continue;
                        }

                        client.apply_action(&action);
                    }
                }
            }
            ServerMessage::Chat(client_id, message) => {
                self.local.other_chat.push_back(ChatMessage {
                    client_id,
                    message,
                    expiry: self.local.time_ms + 5000,
                });
            }
        }
    }
}
