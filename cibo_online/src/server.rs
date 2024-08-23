use crate::{
    client::{ClientGameState, ClientMessage},
    BoxedNetworkObject, Client, ClientAction, ClientId, CollisionInfo, CollisionTester,
    NetworkObjectId, Object, ObjectId, WorldState,
};

use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use monos_gfx::Rect;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

pub struct ServerGameState<T> {
    world: WorldState,
    notify_client: Box<dyn Fn(&T, ServerMessage) + Send + Sync>,
    client_mapping: Vec<(ClientId, T)>,
    queued_moves: Vec<(ClientId, ClientAction)>,
}

impl<T> core::fmt::Debug for ServerGameState<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ServerGameState")
            .field("world", &self.world)
            .field("client_mapping", &self.client_mapping.len())
            .field("queued_moves", &self.queued_moves.len())
            .finish()
    }
}

enum NotifyTarget {
    All,
    AllExcept(ClientId),
    Only(ClientId),
}

impl<T> ServerGameState<T> {
    pub fn new<F>(notify_client: F) -> Self
    where
        F: Fn(&T, ServerMessage) + Send + Sync + 'static,
    {
        ServerGameState {
            world: WorldState::new(),
            notify_client: Box::new(notify_client),
            client_mapping: Vec::new(),
            queued_moves: Vec::new(),
        }
    }

    pub fn new_client(&mut self, id: ClientId, data: T) {
        self.client_mapping.push((id, data));
    }

    pub fn remove_client(&mut self, client_id: ClientId) {
        self.client_mapping.retain(|(id, _)| *id != client_id);
        self.world.clients.retain(|c| c.id() != client_id);

        self.notify_clients(
            ServerMessage::ClientLeft(client_id),
            NotifyTarget::AllExcept(client_id),
        );
    }

    pub fn tick(&mut self, delta_ms: u64) {
        let mut messages = Vec::new();

        struct CollectedHitbox {
            id: ObjectId,
            hitbox: Rect,
            info: CollisionInfo,
        }
        let hitboxes = self
            .world
            .network_objects
            .iter()
            .filter_map(|(id, object)| {
                Some(CollectedHitbox {
                    id: *id,
                    hitbox: object.hitbox()?,
                    info: object.collision_info(),
                })
            })
            .collect::<Vec<_>>();

        let mut collisions = Vec::new();

        for (id, object) in self.world.network_objects.iter_mut() {
            let mut collision_tester = |object: &mut dyn Object| {
                let hitbox = object.hitbox()?;
                let collision_info = object.collision_info();

                hitboxes
                    .iter()
                    .filter_map(|other| {
                        if *id == other.id {
                            return None;
                        }

                        if hitbox.intersects(&other.hitbox) {
                            collisions.push((other.id, collision_info));
                            object.on_collision(other.info);
                            Some(other.info)
                        } else {
                            None
                        }
                    })
                    .next()
            };
            object.tick(delta_ms, CollisionTester::new(&mut collision_tester));
        }

        for (id, info) in collisions {
            if let Some(object) = self.world.network_objects.get_mut(&id) {
                object.on_collision(info);
            }
        }

        for (id, object) in self.world.network_objects.iter_mut() {
            if let Ok(Some(data)) = object.server_tick() {
                messages.push((*id, data))
            }
        }

        for (id, msg) in messages {
            self.notify_clients(ServerMessage::UpdateObject(id, msg), NotifyTarget::All);
        }

        if self.queued_moves.is_empty() {
            return;
        }

        self.notify_clients(
            ServerMessage::UpdateState(self.queued_moves.clone()),
            NotifyTarget::All,
        );

        let mut clients = self.world.clients.iter_mut();
        for queued in self.queued_moves.drain(..) {
            if let Some(client) = clients.find(|c| c.id() == queued.0) {
                client.apply_action(&queued.1);
            }
        }
    }

    pub fn update(&mut self, client_id: ClientId, client_msg: ClientMessage) {
        match client_msg {
            ClientMessage::Connect { mut name } => {
                name.truncate(crate::NAME_LIMIT);
                let mut name = name.trim().to_string();
                if name.is_empty() {
                    name = "Anon".to_string();
                }

                if self.world.clients.iter().any(|c| c.id() == client_id) {
                    return;
                }

                let client = Client::new(client_id, name, Default::default());
                self.world.clients.push(client.clone());

                self.notify_clients(
                    ServerMessage::FullState(SerializedClientGameState::new(
                        client_id,
                        &self.world,
                    )),
                    NotifyTarget::Only(client_id),
                );

                self.notify_clients(
                    ServerMessage::NewClient(client),
                    NotifyTarget::AllExcept(client_id),
                );
            }
            ClientMessage::Action(action) => {
                if let Some((_, existing_action)) = self
                    .queued_moves
                    .iter_mut()
                    .find(|(id, _)| *id == client_id)
                {
                    existing_action.combine(&action);
                } else {
                    self.queued_moves.push((client_id, action));
                }
            }
            ClientMessage::Chat(mut message) => {
                message.truncate(crate::MESSAGE_LIMIT);
                self.notify_clients(ServerMessage::Chat(client_id, message), NotifyTarget::All)
            }
            ClientMessage::UpdateObject(id, data) => {
                let object = match self.world.network_objects.get_mut(&id) {
                    Some(object) => object,
                    None => return,
                };

                match object.server_message(&data) {
                    Ok(Some(msg)) => {
                        self.notify_clients(ServerMessage::UpdateObject(id, msg), NotifyTarget::All)
                    }
                    Ok(None) => {}
                    Err(_) => {}
                }
            }
        }
    }

    pub fn get_special_event(&self, event: SpecialEvent) -> bool {
        self.world.get_special_event(event)
    }
    pub fn set_special_event(&mut self, event: SpecialEvent, active: bool) {
        use crate::world::objects::*;

        match event {
            SpecialEvent::BeachEpisode => {
                let beach_ball_id: NetworkObjectId =
                    crate::get_network_object_id::<BeachBall>().unwrap();

                if active {
                    use rand::Rng;
                    let mut rng = rand::rngs::SmallRng::seed_from_u64(0);
                    for _ in 0..500 {
                        self.add_network_object(BeachBall::new(monos_gfx::Position::new(
                            rng.gen_range(-2000..2000),
                            rng.gen_range(-1000..1000),
                        )));
                    }
                } else {
                    let removed_ids = self
                        .world
                        .network_objects
                        .iter()
                        .filter_map(|(id, object)| {
                            if object.id() == beach_ball_id {
                                Some(*id)
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();

                    for id in removed_ids {
                        self.remove_network_object(id);
                    }
                }
            }
        }

        self.world.set_special_event(event, active);
        self.notify_clients(
            ServerMessage::SpecialEvent { event, active },
            NotifyTarget::All,
        );
    }

    fn add_network_object(&mut self, object: BoxedNetworkObject) -> ObjectId {
        let id = ObjectId::new();
        self.notify_clients(
            ServerMessage::NewObject(id, SerializedNetworkObject::new(&object)),
            NotifyTarget::All,
        );
        self.world.network_objects.insert(id, object);
        id
    }

    fn remove_network_object(&mut self, id: ObjectId) {
        self.world.network_objects.remove(&id);
        self.notify_clients(ServerMessage::DeleteObject(id), NotifyTarget::All);
    }

    fn notify_clients(&self, msg: ServerMessage, target: NotifyTarget) {
        for (id, data) in &self.client_mapping {
            match target {
                NotifyTarget::All => (self.notify_client)(data, msg.clone()),
                NotifyTarget::AllExcept(except_id) if *id != except_id => {
                    (self.notify_client)(data, msg.clone())
                }
                NotifyTarget::Only(target_id) if *id == target_id => {
                    (self.notify_client)(data, msg);
                    break;
                }
                _ => {}
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    FullState(SerializedClientGameState),

    NewClient(Client),
    ClientLeft(ClientId),
    UpdateState(Vec<(ClientId, ClientAction)>),
    Chat(ClientId, String),

    SpecialEvent { event: SpecialEvent, active: bool },

    NewObject(ObjectId, SerializedNetworkObject),
    UpdateObject(ObjectId, Vec<u8>),
    DeleteObject(ObjectId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedClientGameState(ClientId, Vec<u8>);
impl SerializedClientGameState {
    fn new(client_id: ClientId, world: &WorldState) -> Self {
        Self(client_id, postcard::to_allocvec(world).unwrap())
    }

    pub fn serialize(self) -> ClientGameState {
        ClientGameState::new(self.0, postcard::from_bytes(&self.1).unwrap())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedNetworkObject(Vec<u8>);
impl SerializedNetworkObject {
    fn new(object: &BoxedNetworkObject) -> Self {
        Self(postcard::to_allocvec(object).unwrap())
    }

    pub fn serialize(self) -> BoxedNetworkObject {
        postcard::from_bytes(&self.0).unwrap()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SpecialEvent {
    BeachEpisode,
}

impl ServerMessage {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }
}
