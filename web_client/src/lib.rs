use std::{cell::RefCell, ptr::addr_of_mut, rc::Rc};

use cibo_online::{
    client::{ClientGameState, ClientMessage},
    server::ServerMessage,
    ClientAction,
};
use monos_gfx::{Framebuffer, FramebufferFormat};
use wasm_bindgen::prelude::*;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

const FB_SIZE: usize =
    cibo_online::GAME_DIMENSIONS.width as usize * cibo_online::GAME_DIMENSIONS.height as usize * 4;

static mut RAW_FB: [u8; FB_SIZE] = [0u8; FB_SIZE];

/// get a mutable reference to the raw framebuffer
///
/// safety: this is safe as long as it is done only once. since we're in a wasm environment, there can be no threading issues.
// yeah, this is kind of horrible but the way the monos_gfx::Framebuffer is designed makes the alternatives really ugly
unsafe fn raw_fb() -> &'static mut [u8; FB_SIZE] {
    unsafe { &mut *addr_of_mut!(RAW_FB) }
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
#[allow(dead_code)]
struct Game {
    width: u32,
    height: u32,
    framebuffer: Framebuffer<'static>,
    local_state: Box<LocalState>, // box to avoid passing to js by value
}

// everything we don't want to pass to JS
struct LocalState {
    ws: WebSocket,
    game_state: Rc<RefCell<Option<ClientGameState>>>,
    movement: Rc<RefCell<Movement>>,
}

struct Movement {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

impl Movement {
    pub fn new() -> Self {
        Self {
            up: false,
            down: false,
            left: false,
            right: false,
        }
    }

    pub fn any(&self) -> bool {
        self.up || self.down || self.left || self.right
    }
}

#[wasm_bindgen]
#[allow(dead_code)]
impl Game {
    pub fn new(server_host: &str) -> Self {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();

        let format = FramebufferFormat {
            bytes_per_pixel: 4,
            stride: cibo_online::GAME_DIMENSIONS.width as u64,
            r_position: 0,
            g_position: 1,
            b_position: 2,
            a_position: Some(3),
        };

        // safety: this is the only time we're accessing the raw framebuffer
        let framebuffer =
            Framebuffer::new(unsafe { raw_fb() }, cibo_online::GAME_DIMENSIONS, format);

        let local_state = Box::new(LocalState {
            ws: WebSocket::new(&format!("ws://{}/ws", server_host)).unwrap(),
            game_state: Rc::new(RefCell::new(None)),
            movement: Rc::new(RefCell::new(Movement::new())),
        });

        let movement = local_state.movement.clone();
        let on_keydown = Closure::<dyn FnMut(_)>::new(move |e: web_sys::KeyboardEvent| {
            let mut movement = movement.borrow_mut();
            match e.key().as_str() {
                "ArrowUp" => movement.up = true,
                "ArrowDown" => movement.down = true,
                "ArrowLeft" => movement.left = true,
                "ArrowRight" => movement.right = true,
                _ => (),
            };
        });

        web_sys::window()
            .unwrap()
            .add_event_listener_with_callback("keydown", on_keydown.as_ref().unchecked_ref())
            .unwrap();
        on_keydown.forget();

        let movement = local_state.movement.clone();
        let on_keyup = Closure::<dyn FnMut(_)>::new(move |e: web_sys::KeyboardEvent| {
            let mut movement = movement.borrow_mut();
            match e.key().as_str() {
                "ArrowUp" => movement.up = false,
                "ArrowDown" => movement.down = false,
                "ArrowLeft" => movement.left = false,
                "ArrowRight" => movement.right = false,
                _ => (),
            };
        });

        web_sys::window()
            .unwrap()
            .add_event_listener_with_callback("keyup", on_keyup.as_ref().unchecked_ref())
            .unwrap();
        on_keyup.forget();

        local_state
            .ws
            .set_binary_type(web_sys::BinaryType::Arraybuffer);
        let game_state = local_state.game_state.clone();
        let on_message = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
            if let Ok(array_buf) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
                let array = js_sys::Uint8Array::new(&array_buf);
                let server_message = ServerMessage::from_bytes(&array.to_vec());
                match server_message {
                    Ok(ServerMessage::FullState(new_state)) => {
                        game_state.replace(Some(new_state));
                    }
                    Ok(message) => {
                        if let Some(ref mut game_state) = *game_state.borrow_mut() {
                            game_state.handle_message(message);
                        }
                    }
                    Err(e) => console_log!("Error deserializing server message: {:#?}", e),
                }
            }
        });
        local_state
            .ws
            .set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();

        let on_error = Closure::<dyn FnMut(_)>::new(move |e: ErrorEvent| {
            console_log!("WebSocket error: {:?}", e);
        });
        local_state
            .ws
            .set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_error.forget();

        let ws = local_state.ws.clone();
        let on_open = Closure::<dyn FnMut()>::new(move || {
            let client_message = ClientMessage::Connect {
                name: "test".to_string(),
            };
            let bytes = client_message.to_bytes().unwrap();
            ws.send_with_u8_array(&bytes).unwrap();
        });
        local_state
            .ws
            .set_onopen(Some(on_open.as_ref().unchecked_ref()));
        on_open.forget();

        Self {
            width: cibo_online::GAME_DIMENSIONS.width,
            height: cibo_online::GAME_DIMENSIONS.height,
            framebuffer,
            local_state,
        }
    }

    pub fn update(&mut self) {
        self.framebuffer.clear();
        self.framebuffer.clear_alpha();
        if let Some(ref mut game_state) = *self.local_state.game_state.borrow_mut() {
            let movement = self.local_state.movement.borrow();
            if movement.any() {
                let mut action = ClientAction::new();
                action.movement(game_state.client.position());
                if movement.left {
                    action.movement.x -= 5;
                }
                if movement.right {
                    action.movement.x += 5;
                }
                if movement.up {
                    action.movement.y -= 5;
                }
                if movement.down {
                    action.movement.y += 5;
                }

                action.movement.x = action.movement.x.max(0);
                action.movement.x = action.movement.x.min(self.width as i64 - 10);
                action.movement.y = action.movement.y.max(0);
                action.movement.y = action.movement.y.min(self.height as i64 - 10);

                game_state.client.apply_action(&action);
                let client_message = ClientMessage::Action(action);
                self.local_state
                    .ws
                    .send_with_u8_array(&client_message.to_bytes().unwrap())
                    .unwrap();
            }

            game_state.render(&mut self.framebuffer);
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
