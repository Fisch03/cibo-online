#![no_std]
#![feature(effects)]
#![feature(const_trait_impl)]

extern crate alloc;

mod world;
use world::{WorldLocalState, WorldState};

mod render;
use render::{
    widgets, Assets, Object, ObjectProperties, RectExt, RenderContext, Renderable, Sprite,
};

pub mod client;
pub use client::{Client, ClientAction, ClientId};

pub mod server;

fn assets() -> &'static Assets {
    // safety: this assumes that the crate is only used in a single-threaded environment
    static mut ASSETS: Option<Assets> = None;
    unsafe { ASSETS.get_or_insert_with(|| Assets::new()) }
}

pub const SERVER_TICK_RATE: u64 = 1000 / 60;
pub const MESSAGE_LIMIT: usize = 100;
pub const NAME_LIMIT: usize = 16;
pub const BASE_ANIM_SPEED: usize = 250;
