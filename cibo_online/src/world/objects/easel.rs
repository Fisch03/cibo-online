use core::ops::Add;

use crate::{assets, Object, ObjectProperties, RectExt, RenderContext, Renderable, Sprite, ZOrder};
use alloc::{boxed::Box, vec, vec::Vec};
use monos_gfx::{
    font,
    input::Key,
    ui::{Direction, MarginMode, UIContext, UIElement, UIFrame, UIResult},
    Color, Dimension, Framebuffer, FramebufferFormat, Position, Rect,
};

#[derive(Debug)]
pub struct Easel {
    properties: ObjectProperties,
    canvas: Option<Canvas>,
    opened: bool,
}

impl Easel {
    pub fn new(position: Position) -> Box<dyn Object> {
        let dimensions = assets().easel.dimensions();

        let hitbox = Rect::new(
            Position::new(0, dimensions.height as i64 - 8),
            Position::from_dimensions(dimensions),
        );
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
            opened: true,
            canvas: None,
        })
    }
}

impl Renderable for Easel {
    type LocalState = ();
    fn render(&mut self, _state: &mut Self::LocalState, camera: Position, ctx: &mut RenderContext) {
        let screen_pos = self.properties.position - camera;

        ctx.fb.draw_img(&assets().easel, screen_pos);

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
            let canvas = self.canvas.get_or_insert_with(|| Canvas::new());

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

    fn set_position(&mut self, position: Position) {
        self.properties.position = position;
    }
}

const CANVAS_FG: Color = Color::new(184, 128, 75);
const PALETTE_TOOLS: [PaletteTool; 7] = [
    PaletteTool::Brush,
    PaletteTool::Smudge,
    PaletteTool::PaintTube(Color::new(255, 0, 0)),
    PaletteTool::PaintTube(Color::new(0, 0, 255)),
    PaletteTool::PaintTube(Color::new(255, 255, 0)),
    PaletteTool::PaintTube(Color::new(255, 255, 255)),
    PaletteTool::PaintTube(Color::new(0, 0, 0)),
];

#[derive(Debug)]
struct Canvas {
    properties: CanvasSize,
    canvas_data: Vec<u8>,
    last_canvas_pos: Option<Position>,

    palette_data: Vec<u8>,
    last_palette_pos: Option<Position>,

    primary_color: (Color, u8),
    secondary_color: (Color, u8),

    ui: UIFrame,
    tool: usize,
    tool_offset: i64,
    prev_tool: usize,
    prev_tool_offset: i64,
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
            scale: 5,
        }
    }

    fn medium() -> Self {
        Self {
            size: Dimension::new(64, 64),
            scale: 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaletteTool {
    Brush,
    PaintTube(Color),
    Smudge,
}

impl Canvas {
    fn new() -> Self {
        let properties = CanvasSize::small();

        let mut palette_data = vec![
            0;
            assets().palette.dimensions().height as usize
                * assets().palette.dimensions().width as usize
                * 4
        ];
        let mut palette_fb = Framebuffer::new(
            palette_data.as_mut_slice(),
            assets().palette.dimensions(),
            FramebufferFormat {
                r_position: 0,
                g_position: 1,
                b_position: 2,
                a_position: Some(3),
                bytes_per_pixel: 4,
                stride: assets().palette.dimensions().width as u64,
            },
        );
        palette_fb.draw_rect(Rect::from_dimensions(palette_fb.dimensions()), CANVAS_FG);

        let mut canvas_data =
            vec![0; properties.size.width as usize * properties.size.height as usize * 3];
        let mut canvas_fb = Framebuffer::new(
            canvas_data.as_mut_slice(),
            properties.size,
            FramebufferFormat {
                r_position: 0,
                g_position: 1,
                b_position: 2,
                a_position: None,
                bytes_per_pixel: 3,
                stride: properties.size.width as u64,
            },
        );
        canvas_fb.draw_rect(
            Rect::from_dimensions(canvas_fb.dimensions()),
            Color::new(255, 255, 255),
        );

        Self {
            canvas_data,
            last_canvas_pos: None,

            palette_data,
            last_palette_pos: None,

            properties,
            primary_color: (Color::new(255, 0, 0), 255),
            secondary_color: (Color::new(255, 255, 255), 255),

            ui: UIFrame::new_stateless(Direction::RightToLeft),
            tool: 0,
            tool_offset: 0,
            prev_tool: 0,
            prev_tool_offset: 0,
        }
    }

    fn handle_canvas(&mut self, ctx: &mut RenderContext) {
        let mut canvas_fb = Framebuffer::new(
            self.canvas_data.as_mut_slice(),
            self.properties.size,
            FramebufferFormat {
                r_position: 0,
                g_position: 1,
                b_position: 2,
                a_position: None,
                bytes_per_pixel: 3,
                stride: self.properties.size.width as u64,
            },
        );

        let canvas_fb_pos = Position::new(
            ctx.fb.dimensions().width as i64 / 2 - self.properties.scaled().width as i64 / 2,
            ctx.fb.dimensions().height as i64 / 2 - self.properties.scaled().height as i64 / 2,
        );
        let scaled_mouse =
            (ctx.input.mouse.position - canvas_fb_pos) / self.properties.scale as i64;

        let canvas_fb_rect = Rect::new(
            Position::zero(),
            Position::from_dimensions(self.properties.size) * self.properties.scale as i64,
        )
        .translate(canvas_fb_pos);

        if canvas_fb_rect.contains(ctx.input.mouse.position) {
            if ctx.input.mouse.left_button.pressed || ctx.input.mouse.right_button.pressed {
                let used_color = if ctx.input.mouse.left_button.pressed {
                    self.primary_color
                } else {
                    self.secondary_color
                };

                if let Some(last_cursor_pos) = self.last_canvas_pos {
                    canvas_fb.draw_line_alpha(
                        last_cursor_pos,
                        scaled_mouse,
                        used_color.0,
                        used_color.1,
                    );
                }

                canvas_fb.draw_pixel_alpha(scaled_mouse, used_color.0, used_color.1);

                self.last_canvas_pos = Some(scaled_mouse);
            } else {
                self.last_canvas_pos = None;
            }
        }

        //ctx.fb.draw_rect(canvas_fb_rect, Color::new(255, 255, 255));
        ctx.fb
            .draw_fb_scaled(&canvas_fb, &canvas_fb_pos, self.properties.scale);
    }

    fn handle_palette(&mut self, ctx: &mut RenderContext) {
        let palette_fb_pos = Position::new(
            ctx.fb.dimensions().width as i64 - assets().palette.dimensions().width as i64,
            ctx.fb.dimensions().height as i64 / 2
                - assets().palette.dimensions().height as i64 / 2
                - assets().paint_tube.dimensions().height as i64 / 2,
        );

        let palette_fb_rect = Rect::new(
            Position::zero(),
            Position::from_dimensions(assets().palette.dimensions()),
        )
        .translate(palette_fb_pos);

        if palette_fb_rect.contains(ctx.input.mouse.position) {
            let scaled_mouse = ctx.input.mouse.position - palette_fb_pos;

            if ctx.input.mouse.left_button.pressed || ctx.input.mouse.right_button.pressed {
                if let Some(mut prev_scaled_mouse) = self.last_palette_pos {
                    // do bresenham line drawing between scaled_mouse and prev_scaled_mouse
                    let mut last_pos = prev_scaled_mouse;
                    let dx = (scaled_mouse.x - prev_scaled_mouse.x).abs();
                    let dy = -(scaled_mouse.y - prev_scaled_mouse.y).abs();
                    let sx = if prev_scaled_mouse.x < scaled_mouse.x {
                        1
                    } else {
                        -1
                    };
                    let sy = if prev_scaled_mouse.y < scaled_mouse.y {
                        1
                    } else {
                        -1
                    };
                    let mut err = dx + dy;

                    loop {
                        self.handle_brush(
                            prev_scaled_mouse,
                            last_pos,
                            !ctx.input.mouse.left_button.pressed,
                        );
                        last_pos = prev_scaled_mouse;

                        if scaled_mouse == prev_scaled_mouse {
                            break;
                        }

                        let e2 = 2 * err;
                        if e2 >= dy {
                            if prev_scaled_mouse.x == scaled_mouse.x {
                                break;
                            }
                            err += dy;
                            prev_scaled_mouse.x += sx;
                        }
                        if e2 <= dx {
                            if prev_scaled_mouse.y == scaled_mouse.y {
                                break;
                            }
                            err += dx;
                            prev_scaled_mouse.y += sy;
                        }
                    }
                } else {
                    self.handle_brush(
                        scaled_mouse,
                        scaled_mouse,
                        !ctx.input.mouse.left_button.pressed,
                    );
                }

                self.last_palette_pos = Some(scaled_mouse);
            } else {
                self.last_palette_pos = None;
            }
        }

        let palette_fb = Framebuffer::new(
            self.palette_data.as_mut_slice(),
            assets().palette.dimensions(),
            FramebufferFormat {
                r_position: 0,
                g_position: 1,
                b_position: 2,
                a_position: Some(3),
                bytes_per_pixel: 4,
                stride: assets().palette.dimensions().width as u64,
            },
        );
        ctx.fb.draw_img(&assets().palette, palette_fb_pos);
        ctx.fb.draw_fb_apply_alpha(&palette_fb, palette_fb_pos);
    }

    fn handle_brush(
        &mut self,
        scaled_mouse: Position,
        prev_scaled_mouse: Position,
        secondary_action: bool,
    ) {
        let mut palette_fb = Framebuffer::new(
            self.palette_data.as_mut_slice(),
            assets().palette.dimensions(),
            FramebufferFormat {
                r_position: 0,
                g_position: 1,
                b_position: 2,
                a_position: Some(3),
                bytes_per_pixel: 4,
                stride: assets().palette.dimensions().width as u64,
            },
        );

        match PALETTE_TOOLS[self.tool] {
            PaletteTool::Brush => {
                let color = palette_fb.get_pixel_alpha(scaled_mouse);
                match secondary_action {
                    false => self.primary_color = color,
                    true => self.secondary_color = color,
                }
            }
            PaletteTool::PaintTube(color) => {
                Self::smudge_brush(
                    &mut palette_fb,
                    scaled_mouse,
                    prev_scaled_mouse,
                    Some(color),
                );

                /*
                use rand::rngs::SmallRng;
                use rand::{Rng, SeedableRng};
                let mut rng = SmallRng::seed_from_u64(
                    prev_scaled_mouse.x.wrapping_add(prev_scaled_mouse.y) as u64,
                );
                for _ in 0..rng.gen_range(2..=5) {
                    let pos = Position::new(
                        scaled_mouse.x + rng.gen_range(-7..=7),
                        scaled_mouse.y + rng.gen_range(-7..=7),
                    );

                    palette_fb.draw_disc_alpha(&pos, rng.gen_range(1..=3), color, 255);
                }
                */

                //*selected_tube = None;
            }
            PaletteTool::Smudge => {
                Self::smudge_brush(&mut palette_fb, scaled_mouse, prev_scaled_mouse, None)
            }
        }
    }

    fn smudge_brush(
        palette_fb: &mut Framebuffer,
        scaled_mouse: Position,
        prev_scaled_mouse: Position,
        inject_color: Option<Color>,
    ) {
        let brush = &assets().smudge_brush;

        let mut smudge_content = Vec::with_capacity(
            brush.dimensions().width as usize * brush.dimensions().height as usize,
        );
        for y in 0..brush.dimensions().height as i64 {
            for x in 0..brush.dimensions().width as i64 {
                let pos = Position::new(
                    prev_scaled_mouse.x + x - brush.dimensions().width as i64 / 2,
                    prev_scaled_mouse.y + y - brush.dimensions().height as i64 / 2,
                );

                if pos.x < 0
                    || pos.y < 0
                    || pos.x >= palette_fb.dimensions().width as i64
                    || pos.y >= palette_fb.dimensions().height as i64
                {
                    smudge_content.push((Color::new(0, 0, 0), 0));
                    continue;
                }

                let mut color = palette_fb.get_pixel_alpha(pos);
                if let Some(inject_color) = inject_color {
                    color.0 = Color::from_slice(&mixbox::lerp(
                        color.0.as_slice(),
                        inject_color.as_slice(),
                        if color.0 == CANVAS_FG { 1.0 } else { 0.1 },
                    ));
                }
                smudge_content.push(color);
            }
        }

        for y in 0..brush.dimensions().height as usize {
            for x in 0..brush.dimensions().width as usize {
                let pos = Position::new(
                    scaled_mouse.x + x as i64 - brush.dimensions().width as i64 / 2,
                    scaled_mouse.y + y as i64 - brush.dimensions().height as i64 / 2,
                );

                if pos.x < 0
                    || pos.y < 0
                    || pos.x >= palette_fb.dimensions().width as i64
                    || pos.y >= palette_fb.dimensions().height as i64
                {
                    continue;
                }

                let brush_val = brush.get_pixel(Position::new(x as i64, y as i64)).r;
                if assets().palette_mask.get_pixel(pos) == Color::new(0, 0, 0) && brush_val > 0 {
                    let curr = palette_fb.get_pixel_alpha(pos);
                    let new = smudge_content[y * brush.dimensions().width as usize + x];
                    if new.0 == CANVAS_FG {
                        continue;
                    }

                    palette_fb.draw_pixel_alpha(
                        pos,
                        Color::from_slice(&mixbox::lerp(
                            curr.0.as_slice(),
                            new.0.as_slice(),
                            (brush_val as f32 / 255.0) * 0.7,
                        )),
                        brush_val.max(curr.1),
                    );
                }
            }
        }
    }
}

impl Renderable for Canvas {
    type LocalState = ();
    fn render(
        &mut self,
        _state: &mut Self::LocalState,
        _camera: Position,
        ctx: &mut RenderContext,
    ) {
        self.handle_canvas(ctx);
        self.handle_palette(ctx);

        self.ui.draw_frame(
            ctx.fb,
            Rect::new(
                Position::new(
                    0,
                    ctx.fb.dimensions().height as i64
                        - assets().paint_tube.dimensions().height as i64
                        + 4,
                ),
                Position::new(
                    ctx.fb.dimensions().width as i64,
                    ctx.fb.dimensions().height as i64,
                ),
            )
            .translate(Position::new(0, 6)),
            ctx.input,
            |ui| {
                ui.gap(0);

                for (i, &tool) in PALETTE_TOOLS.iter().enumerate().rev() {
                    let selected = i == self.tool;
                    let prev_selected = i == self.prev_tool;
                    if ui
                        .add(PaletteToolWidget::new(
                            tool,
                            if selected {
                                self.tool_offset
                            } else if prev_selected {
                                self.prev_tool_offset
                            } else {
                                0
                            },
                        ))
                        .clicked
                    {
                        if !selected {
                            self.prev_tool = self.tool;
                            self.tool = i;
                            self.tool_offset = 0;
                            self.prev_tool_offset = 6;
                        }
                    }
                }

                self.tool_offset = self.tool_offset.add(1).min(6);
                self.prev_tool_offset = self.prev_tool_offset.add(-1).max(0);
            },
        );
    }
}

#[derive(Debug)]
pub struct PaletteToolWidget {
    tool: PaletteTool,
    offset: i64,
}

impl PaletteToolWidget {
    fn new(tool: PaletteTool, offset: i64) -> PaletteToolWidget {
        Self { tool, offset }
    }
}

impl UIElement for PaletteToolWidget {
    fn draw(self, context: &mut UIContext) -> UIResult {
        let img = match self.tool {
            PaletteTool::Brush => &assets().paint_tube,
            PaletteTool::PaintTube(_) => &assets().paint_tube,
            PaletteTool::Smudge => &assets().spatula,
        };

        let result = context.alloc_space(img.dimensions());
        let pos = result.rect.min + Position::new(0, -self.offset);

        context.fb.draw_img(img, pos);

        match self.tool {
            PaletteTool::Brush => (),
            PaletteTool::PaintTube(color) => {
                let mut fill = assets().paint_tube_mask.clone();
                fill.set_opaque_color(color);
                context.fb.draw_img(&fill, pos);
            }
            PaletteTool::Smudge => (),
        }

        result
    }
}
