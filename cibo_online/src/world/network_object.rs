use crate::{render::Sprite, Object};
use alloc::{boxed::Box, vec::Vec};
use core::{
    any::TypeId,
    ops::Deref,
    sync::atomic::{AtomicU64, Ordering},
};
use hashbrown::HashMap;
use rustc_hash::FxBuildHasher;
use serde::{
    de::{DeserializeSeed, Visitor},
    ser::SerializeSeq,
    Deserialize, Serialize,
};

type DeserializeFn =
    fn(&mut dyn erased_serde::Deserializer) -> erased_serde::Result<Box<dyn NetworkObject>>;

static NETWORK_OBJ_ID: AtomicU64 = AtomicU64::new(0);
// safety: this assumes that the crate is only used in a single-threaded environment
static mut TYPEID_TO_NETWORK_OBJECT_ID: Option<HashMap<TypeId, NetworkObjectId, FxBuildHasher>> =
    None;
static mut NETWORK_OBJ_ID_TO_DESERIALIZE_FN: Option<
    HashMap<NetworkObjectId, DeserializeFn, FxBuildHasher>,
> = None;
pub fn register_network_object<T: NetworkObject + 'static>(deserialize_fn: DeserializeFn) {
    // safety: this assumes that the crate is only used in a single-threaded environment
    let map = unsafe { TYPEID_TO_NETWORK_OBJECT_ID.get_or_insert_with(|| HashMap::default()) };
    let id = NetworkObjectId::new();
    map.insert(TypeId::of::<T>(), id);

    // safety: this assumes that the crate is only used in a single-threaded environment
    let map = unsafe { NETWORK_OBJ_ID_TO_DESERIALIZE_FN.get_or_insert_with(|| HashMap::default()) };
    map.insert(id, deserialize_fn);
}

pub fn get_network_object_id<T: NetworkObject + 'static>() -> Option<NetworkObjectId> {
    // safety: this assumes that the crate is only used in a single-threaded environment
    let map = unsafe { TYPEID_TO_NETWORK_OBJECT_ID.get_or_insert_with(|| HashMap::default()) };
    map.get(&TypeId::of::<T>()).copied()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct NetworkObjectId(u64);
impl NetworkObjectId {
    pub fn new() -> Self {
        NetworkObjectId(NETWORK_OBJ_ID.fetch_add(1, Ordering::SeqCst))
    }
}

/// serializable object that can optionally also send/receive custom messages
pub trait NetworkObject
where
    Self: Object + Send + Sync + erased_serde::Serialize + 'static,
{
    /// what the server should do when it receives a message from the client. the default implementation does nothing
    ///
    /// `data` is the message data
    /// if returns `Some`, the server will send the data to all clients
    #[allow(unused_variables)]
    fn server_message(&mut self, data: &[u8]) -> Result<Option<Vec<u8>>, postcard::Error> {
        Ok(None)
    }

    /// what the client should do when it receives a message from the server. the default implementation panics
    ///
    /// `data` is the message data
    #[allow(unused_variables)]
    fn client_message(&mut self, data: &[u8]) -> Result<(), postcard::Error> {
        panic!("{:?} received unexpected client message", self);
    }

    /// what the server should do every tick. the default implementation does nothing
    ///
    /// if returns `Some`, the server will send the data to all clients
    #[allow(unused_variables)]
    fn server_tick(&mut self) -> Result<Option<Vec<u8>>, postcard::Error> {
        Ok(None)
    }

    /// what the client should do every tick. the default implementation does nothing
    ///
    /// if returns `Some`, the client will send the data to the server
    fn client_tick(&mut self) -> Result<Option<Vec<u8>>, postcard::Error> {
        Ok(None)
    }
}
erased_serde::serialize_trait_object!(NetworkObject);

/// mostly identical to a `Box<dyn NetworkObject>` but can be serialized and deserialized
#[derive(Debug)]
pub struct BoxedNetworkObject {
    id: NetworkObjectId,
    object: Box<dyn NetworkObject>,
}

impl BoxedNetworkObject {
    pub fn new<T>(object: T) -> Self
    where
        T: NetworkObject + 'static,
    {
        Self {
            id: get_network_object_id::<T>().expect("network object not registered"),
            object: Box::new(object),
        }
    }

    pub fn id(&self) -> NetworkObjectId {
        self.id
    }

    pub fn as_sprite(&mut self) -> Sprite {
        self.object.as_sprite()
    }

    pub fn as_object(&mut self) -> &mut dyn Object {
        &mut *self.object
    }
}

impl Deref for BoxedNetworkObject {
    type Target = dyn NetworkObject;

    fn deref(&self) -> &Self::Target {
        &*self.object
    }
}

impl core::ops::DerefMut for BoxedNetworkObject {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.object
    }
}

impl Serialize for BoxedNetworkObject {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&self.id)?;
        seq.serialize_element(&self.object)?;
        seq.end()
    }
}

impl<'de> Deserialize<'de> for BoxedNetworkObject {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(BoxedNetworkObjectVisitor)
    }
}

struct DeserializeFnApplicator {
    deserialize_fn: DeserializeFn,
}
impl<'de> DeserializeSeed<'de> for DeserializeFnApplicator {
    type Value = Box<dyn NetworkObject>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut erased = <dyn erased_serde::Deserializer>::erase(deserializer);
        (self.deserialize_fn)(&mut erased).map_err(serde::de::Error::custom)
    }
}

struct BoxedNetworkObjectVisitor;
impl<'de> Visitor<'de> for BoxedNetworkObjectVisitor {
    type Value = BoxedNetworkObject;

    fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        formatter.write_str("a boxed network object")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let id = seq
            .next_element::<NetworkObjectId>()?
            .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
        let deserialize_fn = {
            // safety: this assumes that the crate is only used in a single-threaded environment
            let map = unsafe {
                NETWORK_OBJ_ID_TO_DESERIALIZE_FN.get_or_insert_with(|| HashMap::default())
            };
            map.get(&id)
                .copied()
                .ok_or_else(|| serde::de::Error::custom("unknown network object id"))?
        };
        let object = seq
            .next_element_seed(DeserializeFnApplicator { deserialize_fn })
            .map_err(serde::de::Error::custom)?
            .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;

        Ok(BoxedNetworkObject { id, object })
    }
}

macro_rules! register_objects {
    ($($object:ty),* $(,)?) => {
        $(
            $crate::world::network_object::register_network_object::<$object>(|deserializer| {
                use serde::Deserialize;
                let object = <$object>::deserialize(deserializer)?;
                Ok(alloc::boxed::Box::new(object))
            });
        )*
    };
}
pub(crate) use register_objects;
