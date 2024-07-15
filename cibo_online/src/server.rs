use crate::{
    client::{ClientGameState, ClientMessage},
    Client, ClientAction, ClientId, GameState,
};

use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use serde::{Deserialize, Serialize};

pub struct ServerGameState<T> {
    game_state: GameState,
    notify_client: Box<dyn Fn(&T, ServerMessage) + Send + Sync>,
    client_mapping: Vec<(ClientId, T)>,
    queued_moves: Vec<(ClientId, ClientAction)>,
}

pub enum NotifyTarget {
    All,
    AllExcept(ClientId),
    Only(ClientId),
}

impl<T> ServerGameState<T> {
    pub fn new<F>(notify_client: F) -> Self
    where
        F: Fn(&T, ServerMessage) + Send + Sync + 'static,
    {
        ServerGameState {
            game_state: GameState::new(),
            notify_client: Box::new(notify_client),
            client_mapping: Vec::new(),
            queued_moves: Vec::new(),
        }
    }

    pub fn new_client(&mut self, id: ClientId, data: T) -> ClientId {
        self.client_mapping.push((id, data));

        id
    }

    pub fn remove_client(&mut self, client_id: ClientId) {
        self.client_mapping.retain(|(id, _)| *id != client_id);
        self.game_state.clients.retain(|c| c.id() != client_id);

        self.notify_clients(
            ServerMessage::ClientLeft(client_id),
            NotifyTarget::AllExcept(client_id),
        );
    }

    pub fn tick(&mut self) {
        self.queued_moves
            .sort_unstable_by(|(a, _), (b, _)| a.cmp(b));

        if self.queued_moves.is_empty() {
            return;
        }

        self.notify_clients(
            ServerMessage::UpdateState(self.queued_moves.clone()),
            NotifyTarget::All,
        );

        let mut clients = self.game_state.clients.iter_mut();
        for queued in self.queued_moves.drain(..) {
            while let Some(client) = clients.next() {
                if client.id() != queued.0 {
                    continue;
                }

                client.apply_action(&queued.1);
            }
        }
    }

    pub fn update(&mut self, client_id: ClientId, client_msg: ClientMessage) {
        match client_msg {
            ClientMessage::Connect { mut name } => {
                name.truncate(crate::NAME_LIMIT);
                let name = name.trim().to_string();
                if self.game_state.clients.iter().any(|c| c.id() == client_id) {
                    return;
                }

                let client = Client::new(client_id, name, Default::default());

                self.notify_clients(
                    ServerMessage::FullState(ClientGameState::new(
                        client.clone(),
                        self.game_state.clone(),
                    )),
                    NotifyTarget::Only(client_id),
                );

                self.game_state.clients.push(client.clone());
                self.game_state.clients.sort_unstable();

                self.notify_clients(
                    ServerMessage::NewClient(client),
                    NotifyTarget::AllExcept(client_id),
                );
            }
            ClientMessage::Action(action) => {
                if let Some((_, existing_action)) = self
                    .queued_moves
                    .iter_mut()
                    .find(|(id, _)| *id == client_id)
                {
                    existing_action.combine(&action);
                } else {
                    self.queued_moves.push((client_id, action));
                }
            }
            ClientMessage::Chat(mut message) => {
                message.truncate(crate::MESSAGE_LIMIT);
                self.notify_clients(ServerMessage::Chat(client_id, message), NotifyTarget::All)
            }
        }
    }

    fn notify_clients(&self, msg: ServerMessage, target: NotifyTarget) {
        for (id, data) in &self.client_mapping {
            match target {
                NotifyTarget::All => (self.notify_client)(data, msg.clone()),
                NotifyTarget::AllExcept(except_id) if *id != except_id => {
                    (self.notify_client)(data, msg.clone())
                }
                NotifyTarget::Only(target_id) if *id == target_id => {
                    (self.notify_client)(data, msg.clone());
                    break;
                }
                _ => {}
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    NewClient(Client),
    ClientLeft(ClientId),
    FullState(ClientGameState),
    UpdateState(Vec<(ClientId, ClientAction)>),
    Chat(ClientId, String),
}

impl ServerMessage {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }
}
