mod objects;

use crate::{
    assets,
    client::{ClientLocal, OwnClient, OwnClientLocal},
    AsSprite, Client, ClientId, RenderContext, Renderable, Sprite,
};

use alloc::{boxed::Box, rc::Rc, string::String, vec, vec::Vec};
use core::cell::RefCell;
use monos_gfx::{Color, Position, Rect};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorldState {
    pub(crate) clients: Vec<Client>,
}

impl WorldState {
    pub fn new() -> Self {
        WorldState {
            clients: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct WorldLocalState {
    pub(crate) own_id: ClientId,
    pub(crate) own_local: Rc<RefCell<OwnClientLocal>>,
    pub(crate) clients: Vec<(ClientId, Rc<RefCell<ClientLocal>>)>,
    pub(crate) objects: Vec<Box<dyn AsSprite>>,
}

impl WorldLocalState {
    pub fn new(own_id: ClientId) -> Self {
        use objects::*;
        let objects = vec![MessageBoard::new(Position::new(
            assets().message_board.dimensions().width as i64 / 2,
            -(assets().message_board.dimensions().height as i64),
        ))];

        WorldLocalState {
            own_id,
            own_local: Rc::new(RefCell::new(OwnClientLocal::default())),
            clients: Vec::new(),
            objects,
        }
    }

    pub fn add_chat(&self, id: ClientId, message: String, expiry: u64) {
        if id == self.own_id {
            self.own_local.borrow_mut().inner.add_chat(message, expiry);
        } else {
            if let Some(local) =
                self.clients.iter().find_map(
                    |(client_id, local)| {
                        if *client_id == id {
                            Some(local)
                        } else {
                            None
                        }
                    },
                )
            {
                local.borrow_mut().add_chat(message, expiry);
            }
        }
    }
}

impl Renderable for WorldState {
    type LocalState = WorldLocalState;
    fn render(&self, state: &mut Self::LocalState, camera: Position, ctx: &mut RenderContext) {
        // draw floor
        let start_tile = camera / 16;
        let fb_tile_size = ctx.fb.dimensions() / 16;

        if ctx.stream_mode {
            ctx.fb.draw_rect(
                &Rect::from_dimensions(ctx.fb.dimensions()),
                &Color::new(0, 255, 0),
            );
        } else {
            for x in start_tile.x - 1..start_tile.x + fb_tile_size.width as i64 + 2 {
                for y in start_tile.y - 1..start_tile.y + fb_tile_size.height as i64 + 2 {
                    let position = Position::new(x * 16, y * 16) - camera;
                    ctx.fb.draw_img(assets().tiles.from_coords(x, y), &position);
                }
            }
        }

        let mut sprites: Vec<Sprite> =
            Vec::with_capacity(self.clients.len() + 1 + state.objects.len());
        sprites.extend(self.clients.iter().map(|client| {
            if client.id() == state.own_id {
                Sprite::OwnClient(OwnClient(client), state.own_local.clone())
            } else {
                Sprite::Client(
                    client,
                    state
                        .clients
                        .iter()
                        .find(|(id, _)| client.id() == *id)
                        .map(|(_, local)| local.clone())
                        .unwrap_or_else(|| {
                            let local = Rc::new(RefCell::new(ClientLocal::default()));
                            state.clients.push((client.id(), local.clone()));
                            state.clients.last().unwrap().1.clone()
                        }),
                )
            }
        }));
        state.objects.iter().for_each(|object| {
            sprites.push(object.as_sprite());
        });

        // TODO: filter out sprites that are not in the visible area
        sprites.sort_unstable_by(|a, b| a.z_order().cmp(&b.z_order()));

        for sprite in sprites.iter_mut() {
            sprite.render(camera, ctx);
        }
    }
}
