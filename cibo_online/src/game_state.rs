use core::sync::atomic::{AtomicU32, Ordering};
use serde::{Deserialize, Serialize};

use alloc::{string::String, vec::Vec};

use monos_gfx::Position;

static CLIENT_ID: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub struct ClientId(u32);

impl ClientId {
    pub fn new() -> Self {
        ClientId(CLIENT_ID.fetch_add(1, Ordering::SeqCst))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    id: ClientId,
    name: String,
    position: Position,
}

impl Client {
    pub fn new(id: ClientId, name: String, position: Position) -> Self {
        Client { id, name, position }
    }

    #[inline]
    pub fn id(&self) -> ClientId {
        self.id
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn position(&self) -> Position {
        self.position
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GameState {
    pub(crate) clients: Vec<Client>,
}

impl GameState {
    pub fn new() -> Self {
        GameState {
            clients: Vec::new(),
        }
    }
}
