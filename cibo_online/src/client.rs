mod chat_widget;
mod render;

use crate::{
    game_state::MoveDirection, server::ServerMessage, Client, ClientAction, ClientId, GameState,
};

use alloc::{collections::VecDeque, format, string::String, vec::Vec};
use monos_gfx::{
    input::{Input, Key, KeyState, RawKey},
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

#[derive(Debug, Clone, Default)]
pub struct ClientLocalState {
    time_ms: u64,
    last_tick: u64,
    last_message: u64,

    render_state: render::RenderState,

    own_chat: Option<String>,
    other_chat: VecDeque<ChatMessage>,
    chat_log: VecDeque<String>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub client_id: ClientId,
    pub message: String,
    pub expiry: u64,
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

    pub fn update(
        &mut self,
        delta_ms: u64,
        framebuffer: &mut Framebuffer,
        input: &mut Input,
        send_msg: &mut dyn FnMut(ClientMessage),
    ) {
        self.local.time_ms += delta_ms;

        let mut client_action = ClientAction::new();

        let tick_amt = (self.local.time_ms - self.local.last_tick) / crate::SERVER_TICK_RATE;
        self.local.last_tick += tick_amt * crate::SERVER_TICK_RATE;

        if self.local.own_chat.is_none() {
            for input in &input.keyboard {
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
                        direction = Some(MoveDirection::None);
                        self.local.own_chat = Some(String::new());
                        client_action.typing(true);
                    }
                    _ => {}
                }

                if let Some(direction) = direction {
                    match input.state {
                        KeyState::Down => {
                            self.client.movement = direction;
                        }
                        KeyState::Up => {
                            if self.client.movement == direction {
                                self.client.movement = MoveDirection::None;
                                client_action.movement(self.client.position, MoveDirection::None);
                            }
                        }
                        _ => {}
                    }
                }
            }

            if self.client.movement != MoveDirection::None {
                let mut position = self.client.position;
                match self.client.movement {
                    MoveDirection::Up => position.y -= 1 * tick_amt as i64,
                    MoveDirection::Down => position.y += 1 * tick_amt as i64,
                    MoveDirection::Left => position.x -= 1 * tick_amt as i64,
                    MoveDirection::Right => position.x += 1 * tick_amt as i64,
                    MoveDirection::None => unreachable!(),
                }
                client_action.movement(position, self.client.movement)
            }

            // remove the return key from the input queue to avoid instantly closing the chat again
            if self.local.own_chat.is_some() {
                input
                    .keyboard
                    .retain(|k| k.key != Key::RawKey(RawKey::Return));
            }
        } else {
            for input in input.keyboard.iter() {
                match input.key {
                    Key::RawKey(RawKey::Escape) if input.state == KeyState::Down => {
                        self.local.own_chat = None;
                        client_action.typing(false);
                    }
                    _ => {}
                }
            }
        }

        let forced_update =
            self.local.time_ms - self.local.last_message > crate::SERVER_TICK_RATE * 15;
        let has_action = client_action.any();
        if has_action || forced_update {
            if forced_update && !has_action {
                client_action.movement(self.client.position, self.client.movement);
                client_action.typing(self.local.own_chat.is_some());
                self.local.last_message = self.local.time_ms;
            }

            self.client.apply_action(&client_action);
            send_msg(ClientMessage::Action(client_action))
        }

        // for client in self.game_state.clients.iter_mut() {
        //     client.predict_movement(tick_amt);
        // }

        self.local
            .other_chat
            .retain(|chat| chat.expiry > self.local.time_ms);

        self.render(framebuffer, input, send_msg);

        input.clear();
    }

    pub fn handle_message(&mut self, msg: ServerMessage) {
        match msg {
            ServerMessage::NewClient(client) => {
                self.game_state.clients.push(client);
                self.game_state.clients.sort_unstable();
            }
            ServerMessage::ClientLeft(client_id) => {
                self.game_state.clients.retain(|c| c.id() != client_id);
                self.local.render_state.cleanup_client(&client_id);
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
                let client = self
                    .game_state
                    .clients
                    .iter_mut()
                    .find(|c| c.id() == client_id);

                let client_name;
                if let Some(client) = client {
                    client.typing = false;
                    client_name = client.name()
                } else {
                    if client_id == self.client.id() {
                        client_name = "You";
                    } else {
                        client_name = "Unknown";
                    }
                };

                self.local
                    .chat_log
                    .push_back(format!("<{}> {}", client_name, message));
                if self.local.chat_log.len() > 256 {
                    self.local.chat_log.pop_front();
                }

                self.local.other_chat.push_back(ChatMessage {
                    client_id,
                    message,
                    expiry: self.local.time_ms + 5000,
                });
            }
        }
    }
}
