use std::any::{Any, TypeId};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Entity — generational index
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    id: u32,
    generation: u32,
}

impl Entity {
    pub fn id(self) -> u32 {
        self.id
    }

    pub fn generation(self) -> u32 {
        self.generation
    }
}

// ---------------------------------------------------------------------------
// ComponentStore — trait object interface for type-erased sparse sets
// ---------------------------------------------------------------------------

trait ComponentStore {
    fn remove_entity(&mut self, id: u32);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

// ---------------------------------------------------------------------------
// SparseSet<T> — per-component storage
// ---------------------------------------------------------------------------

struct SparseSet<T> {
    sparse: Vec<u32>,
    dense: Vec<u32>,
    data: Vec<T>,
}

const EMPTY: u32 = u32::MAX;

impl<T: 'static> SparseSet<T> {
    fn new() -> Self {
        Self {
            sparse: Vec::new(),
            dense: Vec::new(),
            data: Vec::new(),
        }
    }

    #[allow(dead_code)]
    fn contains(&self, id: u32) -> bool {
        let idx = id as usize;
        idx < self.sparse.len() && self.sparse[idx] != EMPTY
    }

    fn insert(&mut self, id: u32, value: T) {
        let idx = id as usize;
        if idx >= self.sparse.len() {
            self.sparse.resize(idx + 1, EMPTY);
        }
        if self.sparse[idx] != EMPTY {
            // Overwrite existing component.
            let dense_idx = self.sparse[idx] as usize;
            self.data[dense_idx] = value;
        } else {
            self.sparse[idx] = self.dense.len() as u32;
            self.dense.push(id);
            self.data.push(value);
        }
    }

    fn remove(&mut self, id: u32) -> Option<T> {
        let idx = id as usize;
        if idx >= self.sparse.len() || self.sparse[idx] == EMPTY {
            return None;
        }
        let dense_idx = self.sparse[idx] as usize;
        self.sparse[idx] = EMPTY;

        // Swap-remove to keep arrays packed.
        let last = self.dense.len() - 1;
        if dense_idx != last {
            let moved_id = self.dense[last] as usize;
            self.sparse[moved_id] = dense_idx as u32;
        }
        self.dense.swap_remove(dense_idx);
        Some(self.data.swap_remove(dense_idx))
    }

    fn get(&self, id: u32) -> Option<&T> {
        let idx = id as usize;
        if idx >= self.sparse.len() || self.sparse[idx] == EMPTY {
            return None;
        }
        Some(&self.data[self.sparse[idx] as usize])
    }

    fn get_mut(&mut self, id: u32) -> Option<&mut T> {
        let idx = id as usize;
        if idx >= self.sparse.len() || self.sparse[idx] == EMPTY {
            return None;
        }
        Some(&mut self.data[self.sparse[idx] as usize])
    }

    fn iter(&self) -> SparseSetIter<'_, T> {
        SparseSetIter {
            dense: &self.dense,
            data: &self.data,
            index: 0,
        }
    }

    fn iter_mut(&mut self) -> SparseSetIterMut<'_, T> {
        SparseSetIterMut {
            dense: &self.dense,
            data: self.data.as_mut_ptr(),
            len: self.data.len(),
            index: 0,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: 'static> ComponentStore for SparseSet<T> {
    fn remove_entity(&mut self, id: u32) {
        self.remove(id);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------
// Iterators
// ---------------------------------------------------------------------------

struct SparseSetIter<'a, T> {
    dense: &'a [u32],
    data: &'a [T],
    index: usize,
}

impl<'a, T> Iterator for SparseSetIter<'a, T> {
    type Item = (u32, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.dense.len() {
            return None;
        }
        let i = self.index;
        self.index += 1;
        Some((self.dense[i], &self.data[i]))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.dense.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl<T> ExactSizeIterator for SparseSetIter<'_, T> {}

struct SparseSetIterMut<'a, T> {
    dense: &'a [u32],
    data: *mut T,
    len: usize,
    index: usize,
    _marker: std::marker::PhantomData<&'a mut T>,
}

impl<'a, T> Iterator for SparseSetIterMut<'a, T> {
    type Item = (u32, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        let i = self.index;
        self.index += 1;
        // SAFETY: each index is visited exactly once, and `len` equals data length.
        let val = unsafe { &mut *self.data.add(i) };
        Some((self.dense[i], val))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len - self.index;
        (remaining, Some(remaining))
    }
}

impl<T> ExactSizeIterator for SparseSetIterMut<'_, T> {}

// Public query iterator wrappers that yield Entity instead of raw id.

/// Iterator over `(Entity, &T)` pairs from a query.
pub struct QueryIter<'a, T> {
    inner: SparseSetIter<'a, T>,
    generations: &'a [u32],
}

impl<'a, T: 'static> Iterator for QueryIter<'a, T> {
    type Item = (Entity, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        let (id, data) = self.inner.next()?;
        let entity = Entity {
            id,
            generation: self.generations[id as usize],
        };
        Some((entity, data))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<T: 'static> ExactSizeIterator for QueryIter<'_, T> {}

/// Iterator over `(Entity, &mut T)` pairs from a query.
pub struct QueryIterMut<'a, T> {
    inner: SparseSetIterMut<'a, T>,
    generations: &'a [u32],
}

impl<'a, T: 'static> Iterator for QueryIterMut<'a, T> {
    type Item = (Entity, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        let (id, data) = self.inner.next()?;
        let entity = Entity {
            id,
            generation: self.generations[id as usize],
        };
        Some((entity, data))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<T: 'static> ExactSizeIterator for QueryIterMut<'_, T> {}

// ---------------------------------------------------------------------------
// EntityAllocator
// ---------------------------------------------------------------------------

struct EntityAllocator {
    generations: Vec<u32>,
    free: Vec<u32>,
    next_id: u32,
}

impl EntityAllocator {
    fn new() -> Self {
        Self {
            generations: Vec::new(),
            free: Vec::new(),
            next_id: 0,
        }
    }

    fn allocate(&mut self) -> Entity {
        if let Some(id) = self.free.pop() {
            Entity {
                id,
                generation: self.generations[id as usize],
            }
        } else {
            let id = self.next_id;
            self.next_id += 1;
            self.generations.push(0);
            Entity { id, generation: 0 }
        }
    }

    fn deallocate(&mut self, entity: Entity) -> bool {
        if !self.is_alive(entity) {
            return false;
        }
        self.generations[entity.id as usize] += 1;
        self.free.push(entity.id);
        true
    }

    fn is_alive(&self, entity: Entity) -> bool {
        let idx = entity.id as usize;
        idx < self.generations.len() && self.generations[idx] == entity.generation
    }
}

// ---------------------------------------------------------------------------
// World — central container
// ---------------------------------------------------------------------------

/// Sparse-set ECS world.
///
/// Manages entity lifetimes and per-component storage. Components can be any
/// `'static` type — no derive macros or registration required.
pub struct World {
    allocator: EntityAllocator,
    stores: HashMap<TypeId, Box<dyn ComponentStore>>,
}

impl World {
    pub fn new() -> Self {
        Self {
            allocator: EntityAllocator::new(),
            stores: HashMap::new(),
        }
    }

    // -- Entity lifecycle ---------------------------------------------------

    pub fn spawn(&mut self) -> Entity {
        self.allocator.allocate()
    }

    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.allocator.deallocate(entity) {
            return false;
        }
        for store in self.stores.values_mut() {
            store.remove_entity(entity.id);
        }
        true
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        self.allocator.is_alive(entity)
    }

    // -- Components ---------------------------------------------------------

    pub fn insert<T: 'static>(&mut self, entity: Entity, component: T) {
        assert!(
            self.is_alive(entity),
            "cannot insert component on dead entity"
        );
        self.storage_mut::<T>().insert(entity.id, component);
    }

    pub fn remove<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        if !self.is_alive(entity) {
            return None;
        }
        self.storage_mut::<T>().remove(entity.id)
    }

    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        if !self.is_alive(entity) {
            return None;
        }
        self.storage::<T>()?.get(entity.id)
    }

    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        if !self.is_alive(entity) {
            return None;
        }
        self.storage_mut::<T>().get_mut(entity.id)
    }

    // -- Queries ------------------------------------------------------------

    pub fn query<T: 'static>(&self) -> QueryIter<'_, T> {
        match self.storage::<T>() {
            Some(set) => QueryIter {
                inner: set.iter(),
                generations: &self.allocator.generations,
            },
            None => QueryIter {
                inner: SparseSetIter {
                    dense: &[],
                    data: &[],
                    index: 0,
                },
                generations: &self.allocator.generations,
            },
        }
    }

    pub fn query_mut<T: 'static>(&mut self) -> QueryIterMut<'_, T> {
        // Split borrow: we need &allocator.generations and &mut stores simultaneously.
        let generations = &self.allocator.generations;
        let set = self
            .stores
            .get_mut(&TypeId::of::<T>())
            .and_then(|b| b.as_any_mut().downcast_mut::<SparseSet<T>>());

        match set {
            Some(s) => QueryIterMut {
                inner: s.iter_mut(),
                generations,
            },
            None => QueryIterMut {
                inner: SparseSetIterMut {
                    dense: &[],
                    data: std::ptr::null_mut(),
                    len: 0,
                    index: 0,
                    _marker: std::marker::PhantomData,
                },
                generations,
            },
        }
    }

    // -- Internal helpers ---------------------------------------------------

    fn storage<T: 'static>(&self) -> Option<&SparseSet<T>> {
        self.stores
            .get(&TypeId::of::<T>())
            .and_then(|b| b.as_any().downcast_ref::<SparseSet<T>>())
    }

    fn storage_mut<T: 'static>(&mut self) -> &mut SparseSet<T> {
        self.stores
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(SparseSet::<T>::new()))
            .as_any_mut()
            .downcast_mut::<SparseSet<T>>()
            .expect("type mismatch in component store")
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Clone)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Debug, PartialEq)]
    struct Health(i32);

    #[derive(Debug, PartialEq)]
    struct Name(String);

    // -- spawn / despawn / generational safety ------------------------------

    #[test]
    fn spawn_returns_unique_entities() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        assert_ne!(a, b);
    }

    #[test]
    fn despawn_marks_entity_dead() {
        let mut world = World::new();
        let e = world.spawn();
        assert!(world.is_alive(e));
        assert!(world.despawn(e));
        assert!(!world.is_alive(e));
    }

    #[test]
    fn despawn_dead_entity_returns_false() {
        let mut world = World::new();
        let e = world.spawn();
        world.despawn(e);
        assert!(!world.despawn(e));
    }

    #[test]
    fn generation_prevents_stale_access() {
        let mut world = World::new();
        let old = world.spawn();
        world.insert(old, Health(100));
        world.despawn(old);

        let new = world.spawn();
        assert_eq!(old.id(), new.id()); // recycled slot
        assert_ne!(old.generation(), new.generation());

        // Old handle must not see new entity's data.
        assert!(!world.is_alive(old));
        assert!(world.get::<Health>(old).is_none());
    }

    // -- insert / get / remove ----------------------------------------------

    #[test]
    fn insert_and_get() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position { x: 1.0, y: 2.0 });
        assert_eq!(world.get::<Position>(e), Some(&Position { x: 1.0, y: 2.0 }));
    }

    #[test]
    fn insert_overwrites_existing() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Health(100));
        world.insert(e, Health(50));
        assert_eq!(world.get::<Health>(e), Some(&Health(50)));
    }

    #[test]
    fn get_mut_modifies_in_place() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Health(100));
        world.get_mut::<Health>(e).unwrap().0 -= 30;
        assert_eq!(world.get::<Health>(e), Some(&Health(70)));
    }

    #[test]
    fn remove_returns_component() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Health(42));
        assert_eq!(world.remove::<Health>(e), Some(Health(42)));
        assert!(world.get::<Health>(e).is_none());
    }

    #[test]
    fn remove_missing_returns_none() {
        let mut world = World::new();
        let e = world.spawn();
        assert!(world.remove::<Health>(e).is_none());
    }

    #[test]
    #[should_panic(expected = "cannot insert component on dead entity")]
    fn insert_on_dead_entity_panics() {
        let mut world = World::new();
        let e = world.spawn();
        world.despawn(e);
        world.insert(e, Health(1));
    }

    #[test]
    fn despawn_cleans_up_all_components() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position { x: 0.0, y: 0.0 });
        world.insert(e, Health(100));
        world.despawn(e);

        let new = world.spawn();
        // Recycled slot must have no leftover components.
        assert!(world.get::<Position>(new).is_none());
        assert!(world.get::<Health>(new).is_none());
    }

    // -- query iteration ----------------------------------------------------

    #[test]
    fn query_iterates_all_components() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        world.insert(a, Health(10));
        world.insert(b, Health(20));

        let mut results: Vec<_> = world.query::<Health>().collect();
        results.sort_by_key(|(e, _)| e.id());
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], (a, &Health(10)));
        assert_eq!(results[1], (b, &Health(20)));
    }

    #[test]
    fn query_mut_modifies_components() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        world.insert(a, Health(10));
        world.insert(b, Health(20));

        for (_, hp) in world.query_mut::<Health>() {
            hp.0 *= 2;
        }

        assert_eq!(world.get::<Health>(a), Some(&Health(20)));
        assert_eq!(world.get::<Health>(b), Some(&Health(40)));
    }

    #[test]
    fn query_empty_world() {
        let world = World::new();
        assert_eq!(world.query::<Health>().count(), 0);
    }

    #[test]
    fn query_exact_size() {
        let mut world = World::new();
        for _ in 0..5 {
            let e = world.spawn();
            world.insert(e, Health(1));
        }
        let iter = world.query::<Health>();
        assert_eq!(iter.len(), 5);
    }

    // -- swap-remove integrity ----------------------------------------------

    #[test]
    fn swap_remove_preserves_other_entries() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        world.insert(a, Position { x: 1.0, y: 0.0 });
        world.insert(b, Position { x: 2.0, y: 0.0 });
        world.insert(c, Position { x: 3.0, y: 0.0 });

        // Remove the middle element.
        world.remove::<Position>(b);

        assert_eq!(world.get::<Position>(a), Some(&Position { x: 1.0, y: 0.0 }));
        assert!(world.get::<Position>(b).is_none());
        assert_eq!(world.get::<Position>(c), Some(&Position { x: 3.0, y: 0.0 }));

        // Query should return exactly 2 entries.
        assert_eq!(world.query::<Position>().count(), 2);
    }

    #[test]
    fn swap_remove_first_element() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        world.insert(a, Health(1));
        world.insert(b, Health(2));

        world.remove::<Health>(a);

        assert!(world.get::<Health>(a).is_none());
        assert_eq!(world.get::<Health>(b), Some(&Health(2)));
    }

    #[test]
    fn swap_remove_last_element() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        world.insert(a, Health(1));
        world.insert(b, Health(2));

        world.remove::<Health>(b);

        assert_eq!(world.get::<Health>(a), Some(&Health(1)));
        assert!(world.get::<Health>(b).is_none());
    }

    // -- multi-component pattern --------------------------------------------

    #[test]
    fn multi_component_access_pattern() {
        let mut world = World::new();

        let player = world.spawn();
        world.insert(player, Position { x: 0.0, y: 0.0 });
        world.insert(player, Health(100));
        world.insert(player, Name("Player".into()));

        let tree = world.spawn();
        world.insert(tree, Position { x: 5.0, y: 5.0 });
        // tree has no Health

        // Iterate positions, filter those that also have health.
        let alive_positions: Vec<_> = world
            .query::<Position>()
            .filter(|(e, _)| world.get::<Health>(*e).is_some())
            .collect();

        assert_eq!(alive_positions.len(), 1);
        assert_eq!(alive_positions[0].0, player);
    }
}
