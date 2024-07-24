use super::RenderContext;
use crate::client::{Client, ClientLocal, OwnClient, OwnClientLocal};
use crate::RectExt;

use alloc::rc::Rc;
use core::cell::RefCell;
use monos_gfx::{Dimension, Image, Position, Rect};

pub trait Renderable {
    type LocalState;

    fn render(&mut self, state: &mut Self::LocalState, camera: Position, ctx: &mut RenderContext);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ZOrder(i64);

impl ZOrder {
    /// create a z position for game elements. usually the same as y position
    pub const fn new(z: i64) -> Self {
        ZOrder(z)
    }

    /// create a z position for UI elements. this is on on top of everything but ui elements with a higher z position
    pub const fn new_ui(z: u8) -> Self {
        let z = u8::MAX - z;
        ZOrder(i64::MAX - z as i64)
    }
}

/// properties of an object that need to be known in advance.
#[derive(Debug, Clone)]
pub struct ObjectProperties {
    pub position: Position,
    pub dimensions: Dimension,
    pub rel_hitbox: Option<Rect>,
    pub rel_bounds: Rect,
    pub interactable: bool,
    pub override_z: Option<ZOrder>,
}

/// objects that can be converted into a sprite and has a hitbox.
pub trait Object
where
    Self: Renderable<LocalState = ()> + core::fmt::Debug,
{
    fn as_sprite(&mut self) -> Sprite;

    fn properties(&self) -> &ObjectProperties;

    fn interacts_with(&self, pos: Position) -> bool {
        if !self.properties().interactable {
            return false;
        }

        self.hitbox()
            .unwrap_or_else(|| self.bounds())
            .interactable(pos)
    }

    /// get the hitbox of this object in world space.
    #[inline]
    fn hitbox(&self) -> Option<Rect> {
        let properties = self.properties();
        properties
            .rel_hitbox
            .map(|hitbox| hitbox.translate(properties.position))
    }

    /// get the bounds of this object in world space.
    #[inline]
    fn bounds(&self) -> Rect {
        self.properties()
            .rel_bounds
            .translate(self.properties().position)
    }
}

/// something that can be rendered and has a z order ("3d" objects).
pub enum Sprite<'a> {
    Client(&'a mut Client, Rc<RefCell<ClientLocal>>),
    OwnClient(OwnClient<'a>, Rc<RefCell<OwnClientLocal>>),
    Object(&'a mut dyn Object),
    Static {
        position: Position,
        image: &'a Image,
    },
}

impl<'a> Sprite<'a> {
    #[inline(always)]
    pub fn z_order(&self) -> ZOrder {
        match self {
            Self::Object(object) => object.properties().override_z.unwrap_or(ZOrder::new(
                object.properties().position.y + object.properties().dimensions.height as i64,
            )),
            _ => ZOrder::new(self.position().y + self.dimensions().height as i64),
        }
    }

    #[inline(always)]
    pub fn dimensions(&self) -> Dimension {
        match self {
            Self::Client(_, _) | Self::OwnClient(_, _) => Dimension::new(32, 32),
            Self::Object(object) => object.properties().dimensions,
            Self::Static { image, .. } => image.dimensions(),
        }
    }

    #[inline(always)]
    pub fn position(&self) -> Position {
        match self {
            Self::Client(client, _) => client.position,
            Self::OwnClient(client, _) => client.0.position,
            Self::Object(object) => object.properties().position,
            Self::Static { position, .. } => *position,
        }
    }

    pub fn render(&mut self, camera: Position, ctx: &mut RenderContext) {
        match self {
            Self::Client(client, local) => client.render(&mut local.borrow_mut(), camera, ctx),
            Self::OwnClient(client, local) => client.render(&mut local.borrow_mut(), camera, ctx),
            Self::Object(object) => object.render(&mut (), camera, ctx),
            Self::Static { position, image } => {
                ctx.fb.draw_img(image, &(*position - camera));
            }
        }
    }
}
