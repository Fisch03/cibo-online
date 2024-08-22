mod network_object;
mod object;
pub(crate) mod objects;

pub(crate) use network_object::{
    get_network_object_id, BoxedNetworkObject, NetworkObject, NetworkObjectId,
};
pub(crate) use object::{CollisionInfo, CollisionTester, Object, ObjectProperties};

use crate::{
    assets,
    client::{ClientLocal, OwnClient, OwnClientLocal},
    server::SpecialEvent,
    Client, ClientId, RenderContext, Renderable, Sprite,
};

use alloc::{boxed::Box, rc::Rc, string::String, vec, vec::Vec};
use core::cell::RefCell;
use hashbrown::HashMap;
use monos_gfx::{Color, Position, Rect};
use rustc_hash::FxBuildHasher;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct WorldState {
    pub(crate) clients: Vec<Client>,
    pub(crate) special_events: SpecialEventState,
    pub(crate) network_objects: HashMap<ObjectId, BoxedNetworkObject, FxBuildHasher>,
}

impl WorldState {
    pub fn new() -> Self {
        objects::setup_network_objects();

        WorldState {
            clients: Vec::new(),
            special_events: SpecialEventState::default(),
            network_objects: HashMap::with_hasher(FxBuildHasher::default()),
        }
    }

    pub(crate) fn get_special_event(&self, event: SpecialEvent) -> bool {
        match event {
            SpecialEvent::BeachEpisode => self.special_events.beach_episode,
        }
    }
    pub fn set_special_event(&mut self, event: SpecialEvent, active: bool) {
        match event {
            SpecialEvent::BeachEpisode => {
                self.special_events.beach_episode = active;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct SpecialEventState {
    pub(crate) beach_episode: bool,
}

#[derive(Debug)]
pub(crate) struct WorldLocalState {
    pub(crate) own_id: ClientId,
    pub(crate) own_local: Rc<RefCell<OwnClientLocal>>,
    pub(crate) clients: Vec<(ClientId, Rc<RefCell<ClientLocal>>)>,
    pub(crate) objects: Vec<Box<dyn Object>>,
}

impl WorldLocalState {
    pub fn new(own_id: ClientId) -> Self {
        //use objects::*;
        let objects = vec![
            /*
            MessageBoard::new(Position::new(
                assets().message_board.dimensions().width as i64 / 2,
                -(assets().message_board.dimensions().height as i64),
            )),
            Easel::new(Position::new(100, 0)),
            */
        ];

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
    fn render(&mut self, state: &mut Self::LocalState, camera: Position, ctx: &mut RenderContext) {
        // draw floor
        let start_tile = camera / 16;
        let fb_tile_size = ctx.fb.dimensions() / 16;

        if ctx.stream_mode {
            ctx.fb.draw_rect(
                Rect::from_dimensions(ctx.fb.dimensions()),
                Color::new(0, 255, 0),
            );
        } else {
            for x in start_tile.x - 1..start_tile.x + fb_tile_size.width as i64 + 2 {
                for y in start_tile.y - 1..start_tile.y + fb_tile_size.height as i64 + 2 {
                    let position = Position::new(x * 16, y * 16) - camera;
                    let tile = if self.special_events.beach_episode {
                        assets().tiles[1].from_coords(x, y)
                    } else {
                        assets().tiles[0].from_coords(x, y)
                    };
                    ctx.fb.draw_img(tile, position);
                }
            }
        }

        let mut sprites: Vec<Sprite> =
            Vec::with_capacity(self.clients.len() + 1 + state.objects.len());
        sprites.extend(self.clients.iter_mut().map(|client| {
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
        state.objects.iter_mut().for_each(|object| {
            sprites.push(object.as_sprite());
        });
        self.network_objects.iter_mut().for_each(|(_, object)| {
            sprites.push(object.as_sprite());
        });

        // TODO: filter out sprites that are not in the visible area
        sprites.sort_unstable_by(|a, b| a.z_order().cmp(&b.z_order()));

        for sprite in sprites.iter_mut() {
            sprite.render(camera, ctx);
        }
    }
}

use core::sync::atomic::{AtomicU32, Ordering};
static OBJECT_ID: AtomicU32 = AtomicU32::new(0);
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ObjectId(u32);

impl ObjectId {
    pub fn new() -> Self {
        ObjectId(OBJECT_ID.fetch_add(1, Ordering::SeqCst))
    }

    pub fn as_u32(&self) -> u32 {
        self.0
    }
}
