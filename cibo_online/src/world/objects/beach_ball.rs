use alloc::vec::Vec;
use core::ops::Not;

use crate::{
    assets, BoxedNetworkObject, CollisionInfo, CollisionTester, NetworkObject, Object,
    ObjectProperties, RenderContext, Renderable, Sprite,
};
use micromath::F32Ext;
use monos_gfx::{Color, Dimension, Position, Rect};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct BeachBall {
    properties: ObjectProperties,
    position_f: (f32, f32),
    velocity: (f32, f32),
    #[serde(skip)]
    angle: f32,
    #[serde(skip)]
    queued_collision: Option<CollisionInfo>,
}

impl BeachBall {
    pub fn new(position: Position) -> BoxedNetworkObject {
        let dimensions = assets().beach_ball.dimensions();
        let bounds = Rect::from_dimensions(dimensions);
        let hitbox = bounds/*Rect::new(
            Position::new(bounds.min.x + 7, bounds.min.y + 2),
            Position::new(bounds.max.x - 7, bounds.max.y),
        )*/;

        BoxedNetworkObject::new(BeachBall {
            properties: ObjectProperties {
                position,
                dimensions,
                rel_hitbox: Some(hitbox),
                rel_bounds: bounds,
                interactable: false,
                override_z: None,
            },
            angle: 0.0,
            velocity: (0.0, 0.0),
            position_f: (position.x as f32, position.y as f32),
            queued_collision: None,
        })
    }

    fn apply_collision(&mut self, collision: CollisionInfo) {
        self.velocity = if collision.is_player() && collision.velocity() != (0.0, 0.0) {
            self.collision_info().apply_with_force(collision, 1.2)
        } else {
            self.collision_info().apply(collision)
        };

        /*
        self.position_f.0 += self.velocity.0;
        self.position_f.1 += self.velocity.1;

        self.angle +=
            (self.velocity.0.abs() + self.velocity.1.abs() * 0.5) * self.velocity.0.signum() * 7.5;

        self.properties.position.x = self.position_f.0 as i64;
        self.properties.position.y = self.position_f.1 as i64;
        */
    }
}

impl Renderable for BeachBall {
    type LocalState = ();
    fn render(&mut self, _state: &mut Self::LocalState, camera: Position, ctx: &mut RenderContext) {
        let screen_pos = self.properties.position - camera;

        self.properties.position =
            Position::new(self.position_f.0 as i64, self.position_f.1 as i64);

        ctx.fb
            .draw_img(&assets().beach_ball.get_image(self.angle), screen_pos);
    }
}

impl Object for BeachBall {
    fn as_sprite(&mut self) -> Sprite {
        Sprite::Object(self)
    }

    fn properties(&self) -> &ObjectProperties {
        &self.properties
    }

    fn collision_info(&self) -> CollisionInfo {
        CollisionInfo::new_dynamic(
            self.properties.position + self.properties.dimensions.center(),
            self.velocity,
        )
    }
    fn on_collision(&mut self, collision: CollisionInfo) {
        self.queued_collision = Some(collision);
    }

    fn set_position(&mut self, position: Position) {
        self.position_f = (position.x as f32, position.y as f32);
    }

    fn tick(&mut self, delta_ms: u64, mut collision_tester: CollisionTester) {
        let passed_ticks = delta_ms as f32 / crate::SERVER_TICK_RATE as f32;
        let blend = 1.0 - 0.05f32.powf(passed_ticks);
        self.velocity.0 *= blend;
        self.velocity.1 *= blend;

        if self.velocity.0.abs() < 0.1 {
            self.velocity.0 = 0.0;
        }
        if self.velocity.1.abs() < 0.1 {
            self.velocity.1 = 0.0;
        }

        collision_tester.test(self);

        self.position_f.0 += self.velocity.0 * passed_ticks;
        self.position_f.1 += self.velocity.1 * passed_ticks;

        self.angle += (self.velocity.0.abs() + self.velocity.1.abs() * 0.5)
            * self.velocity.0.signum()
            * 7.5
            * passed_ticks;

        self.properties.position.x = self.position_f.0 as i64;
        self.properties.position.y = self.position_f.1 as i64;
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BeachBallStateMessage {
    position: (f32, f32),
    velocity: (f32, f32),
}

impl NetworkObject for BeachBall {
    fn server_message(&mut self, data: &[u8]) -> Result<Option<Vec<u8>>, postcard::Error> {
        let collision: CollisionInfo = postcard::from_bytes(data)?;
        self.apply_collision(collision);

        Ok(Some(postcard::to_allocvec(&BeachBallStateMessage {
            position: self.position_f,
            velocity: self.velocity,
        })?))
    }

    fn client_message(&mut self, data: &[u8]) -> Result<(), postcard::Error> {
        let state: BeachBallStateMessage = postcard::from_bytes(data)?;
        self.position_f = state.position;
        self.velocity = state.velocity;

        Ok(())
    }

    fn server_tick(&mut self) -> Result<Option<Vec<u8>>, postcard::Error> {
        let mut result = None;

        if let Some(collision) = self.queued_collision.take() {
            self.apply_collision(collision);
        }

        if self.velocity.0.abs() > 0.1 || self.velocity.1.abs() > 0.1 {
            result = Some(postcard::to_allocvec(&BeachBallStateMessage {
                position: self.position_f,
                velocity: self.velocity,
            })?);
        }

        Ok(result)
    }

    fn client_tick(&mut self) -> Result<Option<Vec<u8>>, postcard::Error> {
        let mut result = None;

        if let Some(collision) = self.queued_collision.take() {
            result = Some(postcard::to_allocvec(&collision)?);
        }

        Ok(result)
    }
}
