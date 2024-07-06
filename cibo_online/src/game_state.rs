use core::sync::atomic::{AtomicU32, Ordering};
use serde::{Deserialize, Serialize};

use alloc::{string::String, vec::Vec};

use monos_gfx::Position;

static CLIENT_ID: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientAction {
    pub movement: Option<Position>,
}

impl ClientAction {
    pub fn new() -> Self {
        ClientAction { movement: None }
    }

    pub fn movement(&mut self, movement: Position) {
        self.movement = Some(movement);
    }

    pub fn any(&self) -> bool {
        self.movement.is_some()
    }

    pub(crate) fn combine(&mut self, action: &ClientAction) {
        if action.movement.is_some() {
            self.movement = action.movement;
        }
    }
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

    pub fn apply_action(&mut self, action: &ClientAction) {
        if let Some(movement) = action.movement {
            self.position = movement;
        }
    }
}

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Client {}

impl PartialOrd for Client {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Client {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id.cmp(&other.id)
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
