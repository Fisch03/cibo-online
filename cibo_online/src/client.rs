use crate::{server::ServerMessage, Client, ClientAction, GameState};

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientGameState {
    pub client: Client,
    game_state: GameState,
}

use monos_gfx::{Color, Position, Rect};
impl ClientGameState {
    pub(crate) fn new(client: Client, mut game_state: GameState) -> Self {
        game_state.clients.retain(|c| c.id() != client.id());

        ClientGameState { client, game_state }
    }

    pub fn handle_action(&mut self, action: ClientAction) {
        self.client.apply_action(&action);
    }

    pub fn render(&self, framebuffer: &mut monos_gfx::Framebuffer) {
        framebuffer.draw_rect(
            &Rect::new(
                Position::new(self.client.position().x, self.client.position().y),
                Position::new(self.client.position().x + 10, self.client.position().y + 10),
            ),
            &Color::new(255, 255, 255),
        );

        self.game_state.render(framebuffer);
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
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    Connect { name: String },
    Action(ClientAction),
}

impl ClientMessage {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }
}
