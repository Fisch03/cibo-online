use super::{Client, ClientMessage};
use crate::{assets, widgets::ChatWidget, RenderContext, Renderable};
use alloc::{collections::VecDeque, format, string::String};

use monos_gfx::{
    text::{font, TextWrap},
    types::*,
    ui::{widgets, Direction, MarginMode, UIFrame},
};

// wrapper around client to make it render as the controlled player
pub struct OwnClient<'a>(pub &'a Client);

#[derive(Debug, Clone)]
pub struct ClientLocal {
    chat: VecDeque<ChatMessage>,
    ui: UIFrame,
}
impl ClientLocal {
    pub fn add_chat(&mut self, message: String, expiry: u64) {
        self.chat.push_back(ChatMessage { message, expiry });
    }
}
impl Default for ClientLocal {
    fn default() -> Self {
        Self {
            chat: VecDeque::new(),
            ui: UIFrame::new(Direction::BottomToTop),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct OwnClientLocal {
    pub inner: ClientLocal,
    pub chat_input: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub message: String,
    pub expiry: u64,
}

impl Renderable for Client {
    type LocalState = ClientLocal;
    fn render(&mut self, state: &mut Self::LocalState, camera: Position, ctx: &mut RenderContext) {
        let screen_position = self.position - camera;
        let anim_frame = ctx.anim_frame();

        ctx.fb.draw_img(
            assets().cibo.get_client_image(self, anim_frame),
            &screen_position,
        );

        let ui_rect = Rect::new(
            Position::new(screen_position.x - 30, -i64::MAX),
            Position::new(screen_position.x + 30 + 32, screen_position.y + 45),
        );

        state.ui.draw_frame(ctx.fb, ui_rect, ctx.input, |ui| {
            ui.margin(MarginMode::Grow);
            ui.label::<font::Glean>(&self.name());

            ui.alloc_space(Dimension::new(0, 26));

            if self.typing {
                let type_text = match anim_frame % 3 {
                    0 => ".",
                    1 => "..",
                    2 => "...",
                    _ => unreachable!(),
                };
                ui.add(ChatWidget::with_id(
                    type_text,
                    &format!("t_{}", self.id.as_u32()),
                ));
            }

            state.chat.retain(|chat| chat.expiry > ctx.time_ms);
            for chat in state.chat.iter().rev().take(3) {
                ui.add(ChatWidget::new(&chat.message));
            }
        })
    }
}

impl Renderable for OwnClient<'_> {
    type LocalState = OwnClientLocal;
    fn render(&mut self, state: &mut Self::LocalState, camera: Position, ctx: &mut RenderContext) {
        let screen_position = self.0.position - camera;

        ctx.fb.draw_img(
            assets().cibo.get_client_image(self.0, ctx.anim_frame()),
            &screen_position,
        );

        let ui_rect = Rect::new(
            Position::new(screen_position.x - 30, -i64::MAX),
            Position::new(screen_position.x + 30 + 32, screen_position.y + 45),
        );

        state.inner.ui.draw_frame(ctx.fb, ui_rect, ctx.input, |ui| {
            ui.margin(MarginMode::Grow);
            ui.label::<font::Glean>(&self.0.name());

            ui.alloc_space(Dimension::new(0, 26));

            if let Some(chat) = &mut state.chat_input {
                let textbox = widgets::Textbox::<font::Glean>::new(chat)
                    .wrap(TextWrap::Enabled { hyphenate: false })
                    .char_limit(crate::MESSAGE_LIMIT);
                if ui.add(textbox).submitted {
                    if !chat.is_empty() {
                        (ctx.send_msg)(ClientMessage::Chat(chat.clone()));
                    }

                    state.chat_input = None;
                }
            }

            state.inner.chat.retain(|chat| chat.expiry > ctx.time_ms);
            for chat in state.inner.chat.iter().rev().take(3) {
                ui.add(ChatWidget::new(&chat.message));
            }
        })
    }
}
