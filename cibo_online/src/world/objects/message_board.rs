use crate::{assets, Object, ObjectProperties, RectExt, RenderContext, Renderable, Sprite};
use alloc::boxed::Box;
use monos_gfx::{
    font::{self, Font},
    ui::{Direction, MarginMode, UIFrame},
    Position, Rect,
};

#[derive(Debug)]
pub struct MessageBoard {
    properties: ObjectProperties,
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
            },
        })
    }
}

impl Renderable for MessageBoard {
    type LocalState = ();
    fn render(&self, _state: &mut Self::LocalState, camera: Position, ctx: &mut RenderContext) {
        let properties = self.properties();

        let screen_pos = properties.position - camera;
        ctx.fb.draw_img(&assets().message_board, &screen_pos);

        if self.hitbox().unwrap().interactable(ctx.player_pos) {
            let mut ui = UIFrame::new_stateless(Direction::BottomToTop);
            let ui_rect = Rect::new(
                Position::new(screen_pos.x, i64::MIN),
                Position::new(
                    screen_pos.x + properties.dimensions.width as i64,
                    screen_pos.y,
                ),
            );
            ui.draw_frame(ctx.fb, ui_rect, ctx.input, |ui| {
                ui.margin(MarginMode::Grow);
                ui.label::<font::Glean>("press e");
            });
        }
    }
}

impl Object for MessageBoard {
    fn as_sprite(&self) -> Sprite {
        Sprite::Object(self)
    }

    fn properties(&self) -> &ObjectProperties {
        &self.properties
    }
}
