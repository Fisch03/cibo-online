use crate::{server::ServerMessage, Client, GameState};

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientGameState {
    pub client: Client,
    game_state: GameState,
}

impl ClientGameState {
    pub(crate) fn new(client: Client, mut game_state: GameState) -> Self {
        game_state.clients.retain(|c| c.id() != client.id());

        ClientGameState { client, game_state }
    }

    pub fn handle_message(&mut self, msg: ServerMessage) {
        match msg {
            ServerMessage::NewClient(client) => {
                self.game_state.clients.push(client);
            }
            ServerMessage::ClientLeft(client_id) => {
                self.game_state.clients.retain(|c| c.id() != client_id);
            }
            ServerMessage::FullState(_) => {
                panic!(
                    "unexpected FullState message. should be handled by the client implementation"
                );
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    Connect { name: String },
}

impl ClientMessage {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }
}
