// A Hashmap that can be queried by either key or value.

use std::{collections::HashMap, hash::Hash};

use crate::world::entity::local_entity::{HostEntity, OwnedLocalEntity, RemoteEntity};

pub struct LocalEntityRecord {
    host: Option<HostEntity>,
    remote: Option<RemoteEntity>,
}

impl LocalEntityRecord {
    pub fn new_with_host(host: HostEntity) -> Self {
        Self {
            host: Some(host),
            remote: None,
        }
    }

    pub fn new_with_remote(remote: RemoteEntity) -> Self {
        Self {
            host: None,
            remote: Some(remote),
        }
    }

    pub(crate) fn host(&self) -> Option<HostEntity> {
        self.host
    }

    pub(crate) fn remote(&self) -> Option<RemoteEntity> {
        self.remote
    }

    pub(crate) fn is_only_remote(&self) -> bool {
        self.host.is_none() && self.remote.is_some()
    }
}

pub struct LocalEntityMap<E: Copy + Eq + Hash> {
    world_to_local: HashMap<E, LocalEntityRecord>,
    host_to_world: HashMap<HostEntity, E>,
    remote_to_world: HashMap<RemoteEntity, E>,
}

impl<E: Copy + Eq + Hash> LocalEntityMap<E> {
    pub fn new() -> Self {
        Self {
            world_to_local: HashMap::new(),
            host_to_world: HashMap::new(),
            remote_to_world: HashMap::new(),
        }
    }

    pub fn insert_with_host_entity(&mut self, world: E, host: HostEntity) {
        self.world_to_local
            .insert(world, LocalEntityRecord::new_with_host(host));
        self.host_to_world.insert(host, world);
    }

    pub fn insert_with_remote_entity(&mut self, world: E, remote: RemoteEntity) {
        self.world_to_local
            .insert(world, LocalEntityRecord::new_with_remote(remote));
        self.remote_to_world.insert(remote, world);
    }

    pub fn get_host_entity(&self, world: &E) -> Option<HostEntity> {
        self.world_to_local
            .get(world)
            .map(|record| record.host)
            .flatten()
    }

    pub fn get_remote_entity(&self, world: &E) -> Option<RemoteEntity> {
        self.world_to_local
            .get(world)
            .map(|record| record.remote)
            .flatten()
    }

    pub fn get_owned_entity(&self, world: &E) -> Option<OwnedLocalEntity> {
        self.world_to_local.get(world).map(|record| {
            if record.remote.is_some() && record.host.is_some() {
                panic!("Can't convert both here");
            } else if record.remote.is_none() && record.host.is_none() {
                panic!("Can't convert neither here");
            } else if let Some(remote_entity) = record.remote {
                remote_entity.copy_to_owned()
            } else if let Some(host_entity) = record.host {
                host_entity.copy_to_owned()
            } else {
                panic!("impossible");
            }
        })
    }

    pub fn world_entity_from_host(&self, host_entity: &HostEntity) -> Option<&E> {
        self.host_to_world.get(host_entity)
    }

    pub fn world_entity_from_remote(&self, remote_entity: &RemoteEntity) -> Option<&E> {
        self.remote_to_world.get(remote_entity)
    }

    pub fn remove_by_world_entity(&mut self, world: &E) -> Option<LocalEntityRecord> {
        let record_opt = self.world_to_local.remove(world);
        if let Some(record) = &record_opt {
            if let Some(host) = record.host {
                self.host_to_world.remove(&host);
            }
            if let Some(remote) = record.remote {
                self.remote_to_world.remove(&remote);
            }
        }
        record_opt
    }

    pub fn contains_world_entity(&self, world: &E) -> bool {
        self.world_to_local.contains_key(world)
    }

    pub fn contains_host_entity(&self, host_entity: &HostEntity) -> bool {
        self.host_to_world.contains_key(host_entity)
    }

    pub fn contains_remote_entity(&self, remote_entity: &RemoteEntity) -> bool {
        self.remote_to_world.contains_key(remote_entity)
    }

    pub fn len(&self) -> usize {
        self.world_to_local.len()
    }

    pub fn is_empty(&self) -> bool {
        self.world_to_local.is_empty()
    }

    pub fn clear(&mut self) {
        self.world_to_local.clear();
        self.host_to_world.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = (&E, &LocalEntityRecord)> {
        self.world_to_local.iter()
    }
}
