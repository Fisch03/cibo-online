#![no_std]

extern crate alloc;

use monos_gfx::Dimension;

mod game_state;
pub(crate) use game_state::GameState;
pub use game_state::{Client, ClientAction, ClientId};

pub mod client;
pub mod server;

pub const GAME_DIMENSIONS: Dimension = Dimension::new(640, 480);
