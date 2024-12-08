use super::{Client, ClientMessage};
use crate::{assets, widgets::ChatWidget, RenderContext, Renderable};
use alloc::{collections::VecDeque, format, string::String};

use monos_gfx::{
    input::{Key, KeyState, RawKey},
    text::{font, Font, TextWrap},
    types::*,
    ui::{
        widgets, Deserialize, Direction, Lines, MarginMode, Serialize, UIContext, UIElement,
        UIFrame, UIResult,
    },
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
            screen_position,
        );

        let ui_rect = Rect::new(
            Position::new(screen_position.x - 30, -i64::MAX),
            Position::new(screen_position.x + 30 + 32, screen_position.y + 45),
        );

        state.ui.draw_frame(ctx.fb, ui_rect, ctx.input, |ui| {
            ui.margin(MarginMode::Grow);
            ui.add(
                widgets::Label::<font::Glean, _>::new(&self.name()).text_color(ctx.main_font_color),
            );

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
            screen_position,
        );

        let ui_rect = Rect::new(
            Position::new(screen_position.x - 30, -i64::MAX),
            Position::new(screen_position.x + 30 + 32, screen_position.y + 45),
        );

        state.inner.ui.draw_frame(ctx.fb, ui_rect, ctx.input, |ui| {
            ui.margin(MarginMode::Grow);
            ui.add(
                widgets::Label::<font::Glean, _>::new(&self.0.name())
                    .text_color(ctx.main_font_color),
            );

            ui.alloc_space(Dimension::new(0, 26));

            if let Some(chat) = &mut state.chat_input {
                let textbox = Textbox::<font::Glean>::new(chat)
                    .wrap(TextWrap::Enabled { hyphenate: false })
                    .text_color(ctx.main_font_color)
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

// TODO: remove this again
use core::marker::PhantomData;

pub struct Textbox<'a, F>
where
    F: Font,
{
    text: &'a mut String,
    wrap: TextWrap,
    char_limit: Option<usize>,
    font: PhantomData<F>,
    text_color: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct TextboxState {
    cursor: usize,
    selection: Option<usize>,
}

impl<'a, F: Font> Textbox<'a, F> {
    pub fn new(text: &'a mut String) -> Self {
        Self {
            text,
            wrap: TextWrap::Disabled,
            font: PhantomData,
            char_limit: None,
            text_color: Color::new(255, 255, 255),
        }
    }

    pub fn wrap(mut self, wrap: TextWrap) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn char_limit(mut self, limit: usize) -> Self {
        self.char_limit = Some(limit);
        self
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = color;
        self
    }
}

impl<F: Font> UIElement for Textbox<'_, F> {
    fn draw(self, context: &mut UIContext) -> UIResult {
        let id = context.next_id();
        let mut state: TextboxState = context.state_get(id).unwrap_or_default();

        let mut submitted = false;

        state.cursor = state.cursor.min(self.text.len());

        //TODO: check for focus
        while let Some(event) = context.input.keyboard.pop_front() {
            match event.state {
                KeyState::Up => continue,
                _ => (),
            }

            match event.key {
                Key::Unicode(c) => {
                    if let Some(limit) = self.char_limit {
                        if self.text.len() >= limit {
                            continue;
                        }
                    }

                    self.text.insert(state.cursor, c);
                    state.cursor += 1;
                }

                Key::RawKey(RawKey::ArrowLeft) => {
                    if state.cursor > 0 {
                        state.cursor -= 1;
                    }
                }
                Key::RawKey(RawKey::ArrowRight) => {
                    if state.cursor < self.text.len() {
                        state.cursor += 1;
                    }
                }

                Key::RawKey(RawKey::Return) => {
                    submitted = true;
                }
                Key::RawKey(RawKey::Backspace) => {
                    if state.cursor > 0 {
                        self.text.remove(state.cursor - 1);
                        state.cursor -= 1;
                    }
                }
                Key::RawKey(RawKey::Delete) => {
                    if state.cursor < self.text.len() {
                        self.text.remove(state.cursor);
                    }
                }

                _ => (),
            }
        }

        let max_dimensions =
            Dimension::new(context.placer.max_width(), context.fb.dimensions().height);

        let lines = Lines::<F>::layout(self.text, self.wrap, max_dimensions);

        let line_dimensions = lines.dimensions();

        let mut result = context.alloc_space(line_dimensions);
        result.submitted = submitted;

        let lines_rect = Rect::centered_in(result.rect, line_dimensions);

        lines.draw(context.fb, lines_rect.min, self.text_color);

        let cursor_pos = lines_rect.min + lines.char_position(state.cursor);
        let cursor_rect = Rect::new(
            cursor_pos,
            Position::new(cursor_pos.x + 1, cursor_pos.y + F::CHAR_HEIGHT as i64),
        );
        context.fb.draw_rect(cursor_rect, self.text_color);

        context.state_insert(id, state);

        result
    }
}
