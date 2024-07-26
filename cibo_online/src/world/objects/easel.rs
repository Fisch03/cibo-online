use crate::{assets, Object, ObjectProperties, RectExt, RenderContext, Renderable, Sprite, ZOrder};
use alloc::{boxed::Box, vec, vec::Vec};
use monos_gfx::{
    font::{self, Font},
    input::Key,
    ui::{Direction, MarginMode, UIFrame},
    Color, Dimension, Framebuffer, FramebufferFormat, Position, Rect,
};

#[derive(Debug)]
pub struct Easel {
    properties: ObjectProperties,
    image: Option<Canvas>,
    opened: bool,
}

impl Easel {
    pub fn new(position: Position) -> Box<dyn Object> {
        let dimensions = Dimension::new(40, 40);

        let hitbox = Rect::new(Position::zero(), Position::from_dimensions(dimensions));
        let bounds = Rect::new(Position::zero(), Position::from_dimensions(dimensions));

        Box::new(Easel {
            properties: ObjectProperties {
                position,
                dimensions,
                rel_hitbox: Some(hitbox),
                rel_bounds: bounds,
                interactable: true,
                override_z: None,
            },
            opened: false,
            image: None,
        })
    }
}

impl Renderable for Easel {
    type LocalState = ();
    fn render(&mut self, _state: &mut Self::LocalState, camera: Position, ctx: &mut RenderContext) {
        let screen_pos = self.properties.position - camera;

        ctx.fb.draw_rect(
            &Rect::from_dimensions(self.properties.dimensions).translate(screen_pos),
            &Color::new(0, 255, 0),
        );

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
                Position::new(screen_pos.x - 100, i64::MIN),
                Position::new(
                    screen_pos.x + self.properties.dimensions.width as i64 + 100,
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
            let canvas = self.image.get_or_insert_with(|| Canvas::new());

            canvas.render(&mut (), camera, ctx);
        }
    }
}

impl Object for Easel {
    fn as_sprite(&mut self) -> Sprite {
        Sprite::Object(self)
    }

    fn properties(&self) -> &ObjectProperties {
        &self.properties
    }
}

#[derive(Debug)]
struct Canvas {
    properties: CanvasSize,
    data: Vec<u8>,
    last_cursor_pos: Option<Position>,
}

#[derive(Debug, Clone, Copy)]
struct CanvasSize {
    size: Dimension,
    scale: u32,
}

impl CanvasSize {
    fn scaled(&self) -> Dimension {
        self.size * self.scale
    }

    fn small() -> Self {
        Self {
            size: Dimension::new(32, 32),
            scale: 10,
        }
    }
}

impl Canvas {
    fn new() -> Self {
        let properties = CanvasSize::small();

        Self {
            data: vec![255; properties.size.width as usize * properties.size.height as usize * 3],
            properties,
            last_cursor_pos: None,
        }
    }
}

impl Renderable for Canvas {
    type LocalState = ();
    fn render(&mut self, _state: &mut Self::LocalState, camera: Position, ctx: &mut RenderContext) {
        let mut image_fb = Framebuffer::new(
            self.data.as_mut_slice(),
            self.properties.size,
            FramebufferFormat {
                a_position: None,
                r_position: 0,
                g_position: 1,
                b_position: 2,
                bytes_per_pixel: 3,
                stride: self.properties.size.width as u64,
            },
        );

        let image_fb_pos = Position::new(
            ctx.fb.dimensions().width as i64 / 2 - self.properties.scaled().width as i64 / 2,
            ctx.fb.dimensions().height as i64 / 2 - self.properties.scaled().height as i64 / 2,
        );

        let scaled_mouse = (ctx.input.mouse.position - image_fb_pos) / self.properties.scale as i64;

        if ctx.input.mouse.left_button.pressed {
            if let Some(last_cursor_pos) = self.last_cursor_pos {
                image_fb.draw_line(&last_cursor_pos, &scaled_mouse, &Color::new(0, 0, 0));
            }

            image_fb.draw_pixel(&scaled_mouse, &Color::new(0, 0, 0));

            self.last_cursor_pos = Some(scaled_mouse);
        } else {
            self.last_cursor_pos = None;
        }

        ctx.fb
            .draw_fb_scaled(&image_fb, &image_fb_pos, self.properties.scale);
    }
}
