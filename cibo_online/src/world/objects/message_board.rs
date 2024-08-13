use crate::{assets, Object, ObjectProperties, RectExt, RenderContext, Renderable, Sprite, ZOrder};
use alloc::boxed::Box;
use monos_gfx::{
    font::{self, Font},
    input::Key,
    text::{Origin, TextWrap},
    ui::{widgets, Direction, MarginMode, UIFrame},
    Color, Position, Rect,
};

#[derive(Debug)]
pub struct MessageBoard {
    properties: ObjectProperties,
    ui: UIFrame,
    opened: bool,
}

impl MessageBoard {
    pub fn new(position: Position) -> Box<dyn Object> {
        let dimensions = assets().message_board.dimensions();

        let hitbox = Rect::new(
            Position::new(0, dimensions.height as i64 - 10),
            Position::from_dimensions(dimensions),
        );

        let bounds = Rect::new(
            Position::new(0, -(font::Glean::CHAR_HEIGHT as i64) * 2),
            Position::from_dimensions(dimensions),
        );

        Box::new(MessageBoard {
            properties: ObjectProperties {
                position,
                dimensions,
                rel_hitbox: Some(hitbox),
                rel_bounds: bounds,
                interactable: true,
                override_z: None,
            },
            ui: UIFrame::new(Direction::TopToBottom),
            opened: false,
        })
    }
}

impl Renderable for MessageBoard {
    type LocalState = ();
    fn render(&mut self, _state: &mut Self::LocalState, camera: Position, ctx: &mut RenderContext) {
        let screen_pos = self.properties.position - camera;
        ctx.fb.draw_img(&assets().message_board, screen_pos);

        if self.hitbox().unwrap().interactable(ctx.player_pos) {
            if ctx.input.key_pressed(Key::Unicode('e')) {
                self.opened = !self.opened;
                if self.opened {
                    self.properties.override_z = Some(ZOrder::new_ui(0));
                } else {
                    self.properties.override_z = None;
                }
            }

            let mut ui = UIFrame::new_stateless(Direction::BottomToTop);
            let ui_rect = Rect::new(
                Position::new(screen_pos.x, i64::MIN),
                Position::new(
                    screen_pos.x + self.properties.dimensions.width as i64,
                    screen_pos.y,
                ),
            );
            ui.draw_frame(ctx.fb, ui_rect, ctx.input, |ui| {
                ui.margin(MarginMode::Grow);
                ui.label::<font::Glean>("press e");
            });
        } else if self.opened {
            self.opened = false;
            self.properties.override_z = None;
        }

        if self.opened {
            ctx.fb.draw_img(
                &assets().message_board_bg,
                Position::new(
                    ctx.fb.dimensions().width as i64 / 2
                        - assets().message_board_bg.dimensions().width as i64 / 2,
                    ctx.fb.dimensions().height as i64 / 2
                        - assets().message_board_bg.dimensions().height as i64 / 2,
                ),
            );

            let text_rect = Rect::new(
                Position::new(
                    ctx.fb.dimensions().width as i64 / 2
                        - (assets().message_board_bg.dimensions().width as i64 - 80) / 2,
                    ctx.fb.dimensions().height as i64 / 2
                        - (assets().message_board_bg.dimensions().height as i64 - 130) / 2,
                ),
                Position::new(
                    ctx.fb.dimensions().width as i64 / 2
                        + (assets().message_board_bg.dimensions().width as i64 - 80) / 2,
                    ctx.fb.dimensions().height as i64 / 2
                        + (assets().message_board_bg.dimensions().height as i64 - 53) / 2,
                ),
            );

            self.ui.draw_frame(ctx.fb, text_rect, ctx.input, |ui| {
                ui.margin(MarginMode::Grow);
                ui.add(
                    widgets::ScrollableLabel::<font::Haeberli, _>::new(
                        include_str!("message_board_text.txt"),
                        Origin::Top,
                    )
                    .wrap(TextWrap::Enabled { hyphenate: false })
                    .scroll_y(text_rect.dimensions().height)
                    .text_color(Color::new(224, 238, 255)),
                );
            })
        }
    }
}

impl Object for MessageBoard {
    fn as_sprite(&mut self) -> Sprite {
        Sprite::Object(self)
    }

    fn properties(&self) -> &ObjectProperties {
        &self.properties
    }
}
