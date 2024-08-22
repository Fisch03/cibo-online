use crate::{RectExt, Renderable, Sprite, ZOrder};

#[allow(unused_imports)]
use micromath::F32Ext;
use monos_gfx::{Dimension, Position, Rect};
use serde::{Deserialize, Serialize};

/// properties of an object that need to be known in advance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectProperties {
    // position needs to stay constant if the object has a hitbox and the position is not synced with the server since collisions are checked on the server side!
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
    #[allow(unused_variables)]
    fn tick(&mut self, delta_ms: u64, collision_tester: CollisionTester) {}

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

    fn collision_info(&self) -> CollisionInfo {
        CollisionInfo::new_static(
            self.properties().position + self.properties().dimensions.center(),
        )
    }
    #[allow(unused_variables)]
    fn on_collision(&mut self, collision: CollisionInfo) {}

    /// get the bounds of this object in world space.
    #[inline]
    fn bounds(&self) -> Rect {
        self.properties()
            .rel_bounds
            .translate(self.properties().position)
    }

    fn set_position(&mut self, position: Position);
}

pub struct CollisionTester<'a> {
    test_fn: &'a mut dyn FnMut(&mut dyn Object) -> Option<CollisionInfo>,
}

impl CollisionTester<'_> {
    pub fn new(
        test_fn: &mut dyn FnMut(&mut dyn Object) -> Option<CollisionInfo>,
    ) -> CollisionTester {
        CollisionTester { test_fn }
    }

    pub fn test(&mut self, object: &mut dyn Object) -> Option<CollisionInfo> {
        (self.test_fn)(object)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CollisionInfo {
    center: Position,
    velocity: Option<(f32, f32)>,
}

impl CollisionInfo {
    pub fn new_dynamic(center: Position, velocity: (f32, f32)) -> Self {
        CollisionInfo {
            center,
            velocity: Some(velocity),
        }
    }

    pub fn new_static(center: Position) -> Self {
        CollisionInfo {
            center,
            velocity: None,
        }
    }

    pub fn is_static(&self) -> bool {
        self.velocity.is_none()
    }

    /// apply the collision self with other and return the new velocity.
    pub fn apply(self, other: CollisionInfo) -> (f32, f32) {
        if self.is_static() {
            return other.velocity.unwrap_or((0.0, 0.0));
        }

        let mut velocity = self.velocity.unwrap();

        let normal = (
            other.center.x as f32 - self.center.x as f32,
            other.center.y as f32 - self.center.y as f32,
        );

        velocity.0 -= normal.0 * 0.03;
        velocity.1 -= normal.1 * 0.03;

        velocity.0 = velocity.0.max(-1.0).min(1.0);
        velocity.1 = velocity.1.max(-1.0).min(1.0);

        velocity
    }
}
