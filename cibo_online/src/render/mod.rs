mod renderable;
pub use renderable::{AsSprite, Renderable, Sprite};

mod assets;
pub use assets::Assets;

use crate::client::ClientMessage;
use monos_gfx::{Framebuffer, Input, Position};

pub struct RenderContext<'a, 'f> {
    pub fb: &'a mut Framebuffer<'f>,
    pub player_pos: Position,
    pub input: &'a mut Input,
    pub time_ms: u64,
    pub stream_mode: bool,
    pub send_msg: &'a mut dyn FnMut(ClientMessage),
}

impl<'a> RenderContext<'a, '_> {
    pub fn anim_frame(&self) -> usize {
        self.time_ms as usize / crate::BASE_ANIM_SPEED
    }
}
