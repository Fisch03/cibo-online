use std::{cell::RefCell, rc::Rc};

use cibo_online::{
    client::{ClientGameState, ClientMessage},
    server::ServerMessage,
};
use monos_gfx::{
    image::SliceReader,
    input::{Input, Key, KeyEvent, KeyState, RawKey},
    text::font,
    ui::{self, widgets::Textbox, UIFrame},
    Color, Dimension, Framebuffer, FramebufferFormat, Image, Position, Rect,
};
use wasm_bindgen::prelude::*;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

/// create a new static framebuffer
fn raw_fb() -> &'static mut Vec<u8> {
    let fb = Box::new(Vec::new());
    let fb = Box::leak(fb);
    &mut *fb
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
#[allow(dead_code)]
struct Game {
    raw_fb: *mut Vec<u8>,
    framebuffer: Framebuffer<'static>,
    local_state: Box<LocalState>, // box to avoid passing to js by value
}

// everything we don't want to pass to JS
struct LocalState {
    ws: WebSocket,
    game_state: Rc<RefCell<Option<ClientGameState>>>,

    input: Rc<RefCell<Input>>,
    ui_frame: UIFrame,
    name_input: String,
    game_logo: Image,
}

fn js_key_to_key(key: &str) -> Option<(Key, bool)> {
    let key = match key {
        "ArrowUp" => Some(Key::RawKey(RawKey::ArrowUp)),
        "ArrowDown" => Some(Key::RawKey(RawKey::ArrowDown)),
        "ArrowLeft" => Some(Key::RawKey(RawKey::ArrowLeft)),
        "ArrowRight" => Some(Key::RawKey(RawKey::ArrowRight)),
        "Backspace" => Some(Key::RawKey(RawKey::Backspace)),
        "Escape" => Some(Key::RawKey(RawKey::Escape)),
        "Enter" => Some(Key::RawKey(RawKey::Return)),
        "Tab" => Some(Key::RawKey(RawKey::Tab)),
        "F1" => Some(Key::RawKey(RawKey::F1)),
        other if other.len() == 1 => {
            let char = other.chars().next().unwrap();
            Some(Key::Unicode(char))
        }
        _ => None,
    };

    let prevent_default = match key {
        Some(Key::RawKey(_)) => true,
        _ => false,
    };

    key.map(|key| (key, prevent_default))
}

#[wasm_bindgen]
#[allow(dead_code)]
impl Game {
    pub fn new(server_host: &str, width: u32, height: u32) -> Self {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();

        console_log!("Initializing game with dimensions {}x{}", width, height);

        let format = FramebufferFormat {
            bytes_per_pixel: 4,
            stride: width as u64,
            r_position: 0,
            g_position: 1,
            b_position: 2,
            a_position: Some(3),
        };

        let framebuffer = raw_fb();
        framebuffer.resize((width * height * format.bytes_per_pixel as u32) as usize, 0);

        // this is all sorts of horrible, but the current design of the Framebuffer type makes it
        // the easiest option. it should be safe though since wasm is always single-threaded
        let raw_fb = framebuffer as *mut Vec<u8>;

        let mut framebuffer = Framebuffer::new(framebuffer, Dimension::new(width, height), format);
        framebuffer.clear_alpha(); // set the alpha channel to be fully visible. we only need to do this once since the program itself does not modify the alpha channel

        let ws_protocol = if web_sys::window().unwrap().location().protocol().unwrap() == "https:" {
            "wss"
        } else {
            "ws"
        };

        let local_state = Box::new(LocalState {
            ws: WebSocket::new(&format!("{}://{}/ws", ws_protocol, server_host)).unwrap(),
            game_state: Rc::new(RefCell::new(None)),
            ui_frame: UIFrame::new(ui::Direction::TopToBottom),
            input: Rc::new(RefCell::new(Input::default())),
            name_input: String::new(),
            game_logo: Image::from_ppm(&SliceReader::new(include_bytes!("../../assets/logo.ppm",)))
                .expect("Failed to load logo"),
        });

        // register input handlers
        let input = local_state.input.clone();
        let on_keydown = Closure::<dyn FnMut(_)>::new(move |e: web_sys::KeyboardEvent| {
            if let Some((key, prevent_default)) = js_key_to_key(&e.key()) {
                input.borrow_mut().keyboard.push_back(KeyEvent {
                    key,
                    state: KeyState::Down,
                });
                if prevent_default {
                    e.prevent_default();
                }
            }
        });
        web_sys::window()
            .unwrap()
            .add_event_listener_with_callback("keydown", on_keydown.as_ref().unchecked_ref())
            .unwrap();
        on_keydown.forget();

        let input = local_state.input.clone();
        let on_keyup = Closure::<dyn FnMut(_)>::new(move |e: web_sys::KeyboardEvent| {
            if let Some((key, prevent_default)) = js_key_to_key(&e.key()) {
                input.borrow_mut().keyboard.push_back(KeyEvent {
                    key,
                    state: KeyState::Up,
                });
                if prevent_default {
                    e.prevent_default();
                }
            }
        });
        web_sys::window()
            .unwrap()
            .add_event_listener_with_callback("keyup", on_keyup.as_ref().unchecked_ref())
            .unwrap();
        on_keyup.forget();

        // register websocket handlers
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
            alert("connection to server failed. please reload the page to try again.\n if this error keeps persisting, you might have already joined under this ip or you might be banned.");
            console_log!("connection error: {:?}", e.error());
            web_sys::window().unwrap().location().reload().unwrap();
        });
        local_state
            .ws
            .set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_error.forget();

        Self {
            framebuffer,
            local_state,
            raw_fb,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        console_log!("Resizing game to {}x{}", width, height);

        let framebuffer = unsafe { &mut *self.raw_fb };
        framebuffer.resize(
            (width * height * self.framebuffer.format().bytes_per_pixel as u32) as usize,
            0,
        );

        let mut format = self.framebuffer.format().clone();
        format.stride = width as u64;

        self.framebuffer = Framebuffer::new(framebuffer, Dimension::new(width, height), format);
        self.framebuffer.clear_alpha();
    }

    pub fn mouse_pos(&mut self, x: i32, y: i32) {
        let mouse = &mut self.local_state.input.borrow_mut().mouse;
        mouse.position = Position::new(x as i64, y as i64);
    }

    pub fn mouse_scroll(&mut self, scroll: i32) {
        let mouse = &mut self.local_state.input.borrow_mut().mouse;
        mouse.scroll += scroll as i64;
    }

    pub fn update(&mut self, delta_ms: f32) {
        self.framebuffer.clear();
        self.framebuffer.clear_alpha();

        let delta_ms = delta_ms.round() as u64;
        if let Some(ref mut game_state) = *self.local_state.game_state.borrow_mut() {
            // we are connected to the server and have received a game state.
            // let the game state handle the rest
            game_state.update(
                delta_ms,
                &mut self.framebuffer,
                &mut self.local_state.input.borrow_mut(),
                &mut |client_msg| {
                    self.local_state
                        .ws
                        .send_with_u8_array(&client_msg.to_bytes().unwrap())
                        .unwrap();
                },
            );
        } else {
            // no game state was received yet, draw a menu to let the player enter their name
            let fb_rect = Rect::from_dimensions(self.framebuffer.dimensions());

            self.framebuffer
                .draw_rect(&fb_rect, &Color::new(100, 100, 100));

            let logo_pos = Position::new(
                (self.framebuffer.dimensions().width as i64
                    - self.local_state.game_logo.dimensions().width as i64)
                    / 2,
                20,
            );
            self.framebuffer
                .draw_img(&self.local_state.game_logo, &logo_pos);

            let mut rect = fb_rect.clone();
            rect.min.y = self.local_state.game_logo.dimensions().height as i64 + 50;

            self.local_state.ui_frame.draw_frame(
                &mut self.framebuffer,
                rect,
                &mut self.local_state.input.borrow_mut(),
                |ui| {
                    ui.margin(ui::MarginMode::Grow);

                    ui.label::<font::Cozette>("please enter a nickname!");
                    ui.label::<font::Glean>("(or leave empty to join anonymously)");

                    let name_input =
                        Textbox::<font::Cozette>::new(&mut self.local_state.name_input)
                            .char_limit(cibo_online::NAME_LIMIT);
                    if ui.add(name_input).submitted {
                        let client_msg = ClientMessage::Connect {
                            name: self.local_state.name_input.clone(),
                        };
                        self.local_state
                            .ws
                            .send_with_u8_array(&client_msg.to_bytes().unwrap())
                            .unwrap();
                        self.local_state.name_input.clear();
                    }
                },
            );

            let mut credits_frame = UIFrame::new_stateless(ui::Direction::BottomToTop);
            credits_frame.draw_frame(
                &mut self.framebuffer,
                fb_rect,
                &mut self.local_state.input.borrow_mut(),
                |ui| {
                    ui.margin(ui::MarginMode::Grow);

                    ui.label::<font::Glean>("made with ♡ by sakanaa");
                },
            );

            let mut controls_frame = UIFrame::new_stateless(ui::Direction::BottomToTop);
            controls_frame.draw_frame(
                &mut self.framebuffer,
                fb_rect,
                &mut self.local_state.input.borrow_mut(),
                |ui| {
                    ui.label::<font::Glean>("- f1 to enter 'stream mode'");
                    ui.label::<font::Glean>("- tab to view player list");
                    ui.label::<font::Glean>("- enter or t to chat");
                    ui.label::<font::Glean>("- arrow keys or w/a/s/d to move");
                    ui.label::<font::Glean>("controls:");
                },
            );

            self.local_state.input.borrow_mut().clear();
        }
    }

    pub fn width(&self) -> u32 {
        self.framebuffer.dimensions().width
    }

    pub fn height(&self) -> u32 {
        self.framebuffer.dimensions().height
    }

    pub fn get_framebuffer(&self) -> *const u8 {
        self.framebuffer.buffer().as_ptr()
    }
}
