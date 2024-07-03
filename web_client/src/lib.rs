use std::ptr::addr_of_mut;

use wasm_bindgen::prelude::*;
use monos_gfx::{Framebuffer, FramebufferFormat, Color, Position, Dimension, fonts, Rect};


const FB_SIZE: usize = cibo_online::GAME_DIMENSIONS.width as usize * cibo_online::GAME_DIMENSIONS.height as usize * 4;

static mut RAW_FB: [u8; FB_SIZE] = [0u8; FB_SIZE];

/// get a mutable reference to the raw framebuffer
///
/// safety: this is safe as long as it is done only once. since we're in a wasm environment, there can be no threading issues.
// yeah, this is kind of horrible but the way the monos_gfx::Framebuffer is designed makes the alternatives really ugly
unsafe fn raw_fb() -> &'static mut [u8; FB_SIZE] {
    unsafe { &mut *addr_of_mut!(RAW_FB) }
}

#[wasm_bindgen]
struct Game {
    width: u32,
    height: u32,
    framebuffer: Framebuffer<'static>,
}

#[wasm_bindgen]
impl Game {
    pub fn new() -> Self {
        let format = FramebufferFormat {
            bytes_per_pixel: 4,
            stride: cibo_online::GAME_DIMENSIONS.width as u64,
            r_position: 0,
            g_position: 1,
            b_position: 2,
            a_position: Some(3)
        };

        // safety: this is the only time we're accessing the raw framebuffer
        let mut framebuffer = Framebuffer::new(unsafe {raw_fb()}, cibo_online::GAME_DIMENSIONS, format);
        framebuffer.clear_alpha();
        framebuffer.draw_str::<fonts::Cozette>(&Color::new(255, 255, 255), "Hello, world!",  &Position::new(0, 0));

        Self {
            width: cibo_online::GAME_DIMENSIONS.width,
            height: cibo_online::GAME_DIMENSIONS.height,
            framebuffer,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn get_framebuffer(&self) -> *const u8 {
        self.framebuffer.buffer().as_ptr()
    }
}
