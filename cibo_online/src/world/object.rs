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
    is_player: bool,
}

impl CollisionInfo {
    pub fn new_dynamic(center: Position, velocity: (f32, f32)) -> Self {
        CollisionInfo {
            center,
            velocity: Some(velocity),
            is_player: false,
        }
    }

    pub fn new_static(center: Position) -> Self {
        CollisionInfo {
            center,
            velocity: None,
            is_player: false,
        }
    }

    pub(crate) fn new_player(center: Position, velocity: (f32, f32)) -> Self {
        CollisionInfo {
            center,
            velocity: Some(velocity),
            is_player: true,
        }
    }

    pub fn is_player(&self) -> bool {
        self.is_player
    }

    pub fn is_static(&self) -> bool {
        self.velocity.is_none()
    }

    pub fn velocity(&self) -> (f32, f32) {
        self.velocity.unwrap_or((0.0, 0.0))
    }

    /// apply the collision self with other and return the new velocity.
    pub fn apply(self, other: CollisionInfo) -> (f32, f32) {
        let normal = (
            other.center.x as f32 - self.center.x as f32,
            other.center.y as f32 - self.center.y as f32,
        );
        let normal_len = (normal.0 * normal.0 + normal.1 * normal.1).sqrt();
        let normal = (normal.0 / normal_len, normal.1 / normal_len);

        self.apply_raw(other, normal, normal_len)
    }

    pub fn apply_with_force(self, other: CollisionInfo, force: f32) -> (f32, f32) {
        let normal = (
            other.center.x as f32 - self.center.x as f32,
            other.center.y as f32 - self.center.y as f32,
        );
        let normal_len = (normal.0 * normal.0 + normal.1 * normal.1).sqrt();
        let normal = (normal.0 / normal_len, normal.1 / normal_len);

        let mut velocity = self.apply_raw(other, normal, normal_len);
        velocity.0 += -normal.0 * force;
        velocity.1 += -normal.1 * force;

        velocity
    }

    fn apply_raw(self, other: CollisionInfo, normal: (f32, f32), normal_len: f32) -> (f32, f32) {
        if self.is_static() {
            return (0.0, 0.0);
        }

        let mut velocity = self.velocity.unwrap();

        if other.is_static() {
            let dot = velocity.0 * normal.0 + velocity.1 * normal.1;
            velocity.0 -= 2.0 * dot * normal.0;
            velocity.1 -= 2.0 * dot * normal.1;
        } else {
            // TODO: this is not correct but the engine gets unstable otherwise. this only works for beach ball sized objects :P
            velocity.0 -= normal.0 * (16.0 / normal_len.max(0.1)) * 0.5;
            velocity.1 -= normal.1 * (16.0 / normal_len.max(0.1)) * 0.5;
        }

        velocity.0 = velocity.0.clamp(-5.0, 5.0);
        velocity.1 = velocity.1.clamp(-5.0, 5.0);

        velocity
    }
}
