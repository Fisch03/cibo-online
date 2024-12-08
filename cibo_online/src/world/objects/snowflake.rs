use crate::{
    assets, rng, Object, ObjectProperties, RectExt, RenderContext, Renderable, Sprite, ZOrder,
};
use alloc::boxed::Box;
use monos_gfx::{
    font::{self, Font},
    input::Key,
    text::{Origin, TextWrap},
    ui::{widgets, Direction, MarginMode, UIFrame},
    Color, Position, Rect,
};
use rand::Rng;

#[derive(Debug)]
pub struct Snowflake {
    properties: ObjectProperties,
    variant: usize,
    speed: f32,
    pos_y: f32,
}

impl Snowflake {
    pub fn new() -> Box<dyn Object> {
        let dimensions = assets().snowflakes[0].dimensions();
        let pos_y = rng().gen_range(-500.0..-50.0);
        let speed = rng().gen_range(0.1..0.7);
        let position = Position::new(rng().gen_range(0..550), pos_y as i64);

        let bounds = Rect::from_dimensions(dimensions);

        Box::new(Snowflake {
            properties: ObjectProperties {
                position,
                dimensions,
                rel_hitbox: None,
                rel_bounds: bounds,
                interactable: false,
                override_z: Some(ZOrder::new_ui(10)),
            },
            variant: rng().gen_range(0..assets().snowflakes.len()),
            speed,
            pos_y,
        })
    }
}

impl Renderable for Snowflake {
    type LocalState = ();
    fn render(&mut self, _state: &mut Self::LocalState, camera: Position, ctx: &mut RenderContext) {
        self.pos_y += self.speed;
        if self.pos_y > 400.0 {
            self.pos_y = camera.y as f32 - rng().gen_range(100..200) as f32;
            self.properties.position.x = camera.x as i64 + rng().gen_range(-550..550);
        }
        self.properties.position.y = self.pos_y as i64;

        ctx.fb.draw_img(
            &assets().snowflakes[self.variant],
            self.properties.position - camera,
        );
    }
}

impl Object for Snowflake {
    fn as_sprite(&mut self) -> Sprite {
        Sprite::Object(self)
    }

    fn properties(&self) -> &ObjectProperties {
        &self.properties
    }

    fn set_position(&mut self, position: Position) {
        self.properties.position = position;
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
