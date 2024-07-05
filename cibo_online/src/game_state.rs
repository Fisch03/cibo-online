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
    pub movement: Position,
}

impl ClientAction {
    pub fn new() -> Self {
        ClientAction {
            movement: Position::new(50, 50),
        }
    }

    pub fn movement(&mut self, movement: Position) {
        self.movement = movement;
    }

    pub fn combine(&mut self, newer: &Self) {
        self.movement = newer.movement;
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
        self.position = action.movement;
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

use monos_gfx::{Color, Framebuffer, Rect};
impl GameState {
    pub fn new() -> Self {
        GameState {
            clients: Vec::new(),
        }
    }

    pub fn render(&self, framebuffer: &mut Framebuffer) {
        for client in &self.clients {
            framebuffer.draw_rect(
                &Rect::new(
                    Position::new(client.position.x, client.position.y),
                    Position::new(client.position.x + 32, client.position.y + 32),
                ),
                &Color::new(255, 255, 255),
            );
        }
    }
}
