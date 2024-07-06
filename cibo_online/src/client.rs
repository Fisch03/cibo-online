use crate::{server::ServerMessage, Client, ClientAction, ClientId, GameState};

use alloc::{collections::VecDeque, string::String, vec::Vec};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientGameState {
    pub client: Client,
    game_state: GameState,

    #[serde(skip)]
    time_ms: u64,
    #[serde(skip)]
    last_tick: u64,
    #[serde(skip)]
    pub input: InputState,
    #[serde(skip)]
    pub chat: VecDeque<ChatMessage>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub client_id: ClientId,
    pub message: String,
    pub expiry: u64,
}

#[derive(Debug, Clone, Default)]
pub struct InputState {
    chat: Option<String>,
    last_movement: MoveDirection,
    movement: MoveDirection,
}

impl InputState {
    pub fn walk(&mut self, direction: MoveDirection) {
        self.last_movement = self.movement;
        self.movement = direction;
    }

    pub fn stop(&mut self, direction: MoveDirection) {
        if self.last_movement == direction {
            self.last_movement = MoveDirection::None;
        }

        if self.movement == direction {
            self.movement = self.last_movement;
        }
    }

    pub fn exit_chat(&mut self) {
        self.chat = None;
    }

    pub fn chat_open(&self) -> bool {
        self.chat.is_some()
    }

    pub fn toggle_chat(&mut self) -> Option<ClientMessage> {
        if let Some(ref chat) = self.chat {
            let message = if !chat.is_empty() {
                Some(ClientMessage::Chat(chat.clone()))
            } else {
                None
            };
            self.chat = None;
            return message;
        } else {
            self.chat = Some(String::new());
        }
        None
    }

    pub fn chat(&mut self, c: char) {
        if let Some(ref mut chat) = self.chat {
            chat.push(c);
        }
    }

    pub fn backspace(&mut self) {
        if let Some(ref mut chat) = self.chat {
            chat.pop();
        }
    }

    fn will_move(&self) -> bool {
        self.chat.is_none() && self.movement != MoveDirection::None
    }
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

use monos_gfx::{Color, Framebuffer, Position, Rect};
impl ClientGameState {
    pub(crate) fn new(client: Client, mut game_state: GameState) -> Self {
        game_state.clients.retain(|c| c.id() != client.id());

        ClientGameState {
            client,
            game_state,

            time_ms: 0,
            last_tick: 0,
            input: InputState::default(),
            chat: VecDeque::new(),
        }
    }

    pub fn handle_action(&mut self, action: ClientAction) {
        self.client.apply_action(&action);
    }

    pub fn update(
        &mut self,
        delta_ms: u64,
        framebuffer: &mut Framebuffer,
    ) -> Option<ClientMessage> {
        self.time_ms += delta_ms;

        let mut client_action = ClientAction::new();

        while self.last_tick < self.time_ms {
            if self.input.will_move() {
                let mut position = self.client.position();
                match self.input.movement {
                    MoveDirection::Up => position.y -= 1,
                    MoveDirection::Down => position.y += 1,
                    MoveDirection::Left => position.x -= 1,
                    MoveDirection::Right => position.x += 1,
                    MoveDirection::None => unreachable!(),
                }
                client_action.movement(position);
            }

            self.last_tick += crate::SERVER_TICK_RATE;
        }

        let client_message = if client_action.any() {
            self.client.apply_action(&client_action);
            Some(ClientMessage::Action(client_action))
        } else {
            None
        };

        framebuffer.draw_rect(
            &Rect::new(
                Position::new(self.client.position().x, self.client.position().y),
                Position::new(self.client.position().x + 32, self.client.position().y + 32),
            ),
            &Color::new(255, 255, 255),
        );

        for client in &self.game_state.clients {
            let position = client.position();
            framebuffer.draw_rect(
                &Rect::new(
                    Position::new(position.x, position.y),
                    Position::new(position.x + 32, position.y + 32),
                ),
                &Color::new(255, 255, 255),
            );
        }

        client_message
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
                self.chat.push_back(ChatMessage {
                    client_id,
                    message,
                    expiry: self.time_ms + 5000,
                });
            }
        }
    }
}

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
