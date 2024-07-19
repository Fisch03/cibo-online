use super::RenderContext;
use crate::client::{Client, ClientLocal, OwnClient, OwnClientLocal};

use alloc::{boxed::Box, rc::Rc};
use core::cell::RefCell;
use monos_gfx::{Dimension, Image, Position, Rect};

pub trait Renderable {
    type LocalState;

    fn render(&self, state: &mut Self::LocalState, camera: Position, ctx: &mut RenderContext);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ZOrder(i64);

impl ZOrder {
    /// create a z position for game elements. usually the same as y position
    pub const fn new(z: i64) -> Self {
        ZOrder(z)
    }

    // create a z position for UI elements. this is on on top of everything but ui elements with a higher z position
    // pub const fn new_ui(z: u8) -> Self {
    //     let z = u8::MAX - z;
    //     ZOrder(i64::MAX - z as i64)
    // }
}

/// objects that can be converted into a sprite. this conversion should be as cheap as possible.
pub trait AsSprite: core::fmt::Debug {
    fn as_sprite(&self) -> Sprite;
    fn position(&self) -> Position {
        self.as_sprite().position()
    }

    fn dimensions(&self) -> Dimension {
        self.as_sprite().dimensions()
    }

    /// get the hitbox of this object relative to its position.
    fn hitbox_rel(&self) -> Option<Rect> {
        let dimension = self.dimensions();
        Some(Rect::new(
            Position::zero(),
            Position::from_dimensions(dimension),
        ))
    }

    /// get the hitbox of this object in world.
    ///
    /// you should implement `hitbox_rel` instead.
    fn hitbox(&self) -> Option<Rect> {
        self.hitbox_rel()
            .map(|hitbox| hitbox.translate(self.position()))
    }
}

/// something that can be rendered and has a z order ("3d" objects).
pub enum Sprite<'a> {
    Client(&'a Client, Rc<RefCell<ClientLocal>>),
    OwnClient(OwnClient<'a>, Rc<RefCell<OwnClientLocal>>),
    Static {
        position: Position,
        image: &'a Image,
    },
    Dynamic {
        position: Position,
        dimension: Dimension,
        on_render: Option<Box<dyn FnOnce(&mut RenderContext) -> Image>>,
    },
}

impl<'a> Sprite<'a> {
    #[inline(always)]
    pub const fn z_order(&self) -> ZOrder {
        match self {
            _ => ZOrder::new(self.position().y + self.dimensions().height as i64),
        }
    }

    #[inline(always)]
    pub const fn dimensions(&self) -> Dimension {
        match self {
            Self::Client(_, _) | Self::OwnClient(_, _) => Dimension::new(32, 32),
            Self::Static { image, .. } => image.dimensions(),
            Self::Dynamic { dimension, .. } => *dimension,
        }
    }

    #[inline(always)]
    pub const fn position(&self) -> Position {
        match self {
            Self::Client(client, _) => client.position,
            Self::OwnClient(client, _) => client.0.position,
            Self::Static { position, .. } | Self::Dynamic { position, .. } => *position,
        }
    }

    pub fn render(&mut self, camera: Position, ctx: &mut RenderContext) {
        match self {
            Self::Client(client, local) => client.render(&mut local.borrow_mut(), camera, ctx),
            Self::OwnClient(client, local) => client.render(&mut local.borrow_mut(), camera, ctx),
            Self::Static { position, image } => {
                ctx.fb.draw_img(image, &(*position - camera));
            }
            Self::Dynamic {
                position,
                on_render,
                ..
            } => {
                if let Some(on_render) = on_render.take() {
                    let image = on_render(ctx);
                    ctx.fb.draw_img(&image, &(*position - camera));
                } else {
                    panic!("Dynamic sprite was rendered twice");
                }
            }
        }
    }
}
