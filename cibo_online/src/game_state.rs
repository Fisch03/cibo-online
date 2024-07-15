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

    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    id: ClientId,
    name: String,
    pub(crate) typing: bool,
    pub(crate) position: Position,
    pub(crate) movement: MoveDirection,
    pub(crate) look_direction: MoveDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientAction {
    pub movement: Option<(Position, MoveDirection)>,
    pub typing: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl ClientAction {
    pub fn new() -> Self {
        ClientAction {
            movement: None,
            typing: None,
        }
    }

    pub fn movement(&mut self, movement: Position, direction: MoveDirection) {
        self.movement = Some((movement, direction));
    }

    pub fn typing(&mut self, typing: bool) {
        self.typing = Some(typing);
    }

    pub fn any(&self) -> bool {
        self.movement.is_some() || self.typing.is_some()
    }

    pub(crate) fn combine(&mut self, action: &ClientAction) {
        if action.movement.is_some() {
            self.movement = action.movement;
        }

        if action.typing.is_some() {
            self.typing = action.typing;
        }
    }
}

impl Client {
    pub fn new(id: ClientId, name: String, position: Position) -> Self {
        Client {
            id,
            name,
            typing: false,
            position,
            movement: MoveDirection::None,
            look_direction: MoveDirection::None,
        }
    }

    #[inline]
    pub fn id(&self) -> ClientId {
        self.id
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn apply_action(&mut self, action: &ClientAction) {
        if let Some(movement) = action.movement {
            self.position = movement.0;
            self.movement = movement.1;
            if movement.1 != MoveDirection::None {
                self.look_direction = movement.1;
            }
        }

        if let Some(typing) = action.typing {
            self.typing = typing
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
