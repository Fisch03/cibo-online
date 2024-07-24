mod renderable;
pub use renderable::{Object, ObjectProperties, Renderable, Sprite, ZOrder};

mod assets;
pub use assets::Assets;

pub mod widgets;

use crate::client::ClientMessage;
use monos_gfx::{Framebuffer, Input, Position, Rect};

pub struct RenderContext<'a, 'f> {
    pub fb: &'a mut Framebuffer<'f>,
    pub player_pos: Position,
    pub input: &'a mut Input,
    pub time_ms: u64,
    pub stream_mode: bool,
    pub send_msg: &'a mut dyn FnMut(ClientMessage),
}

impl<'a> RenderContext<'a, '_> {
    #[inline(always)]
    pub fn anim_frame(&self) -> usize {
        self.time_ms as usize / crate::BASE_ANIM_SPEED
    }
}

pub trait RectExt {
    fn interactable(&self, pos: Position) -> bool;
}

impl RectExt for Rect {
    fn interactable(&self, pos: Position) -> bool {
        pos.x > self.min.x - 16
            && pos.x < self.max.x - 16
            && pos.y > self.max.y - 30
            && pos.y < self.max.y + 10
    }
}
