mod chat_widget;
use chat_widget::ChatWidget;

use super::{ClientGameState, ClientLocalState, ClientMessage, MoveDirection};
use crate::{Client, ClientId};
use alloc::{boxed::Box, format, rc::Rc, vec::Vec};
use core::cell::RefCell;

use monos_gfx::{
    image::SliceReader,
    input::{Input, Key, KeyState, RawKey},
    text::{font, Origin},
    types::*,
    ui::{widgets, Direction, MarginMode, TextWrap, UIFrame},
    Framebuffer, Image,
};

const CAMERA_EDGE: i64 = 100;
const ANIM_FRAME_DURATION: usize = 250;

#[derive(Debug, Clone)]
pub struct RenderState {
    camera: Position,
    chat_log: UIFrame,
    coordinate_display: UIFrame,
    player_list: Option<UIFrame>,
    stream_mode: bool,
}

impl Default for RenderState {
    fn default() -> Self {
        let camera = Position::new(0, 0);

        Self {
            camera,
            chat_log: UIFrame::new(Direction::BottomToTop),
            coordinate_display: UIFrame::new_stateless(Direction::RightToLeft),
            player_list: None,
            stream_mode: false,
        }
    }
}

impl ClientGameState {
    pub(super) fn render(
        &mut self,
        framebuffer: &mut Framebuffer,
        input: &mut Input,
        send_msg: &mut dyn FnMut(ClientMessage),
    ) {
    }
}
