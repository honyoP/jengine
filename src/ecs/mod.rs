use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;

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

// These types are `pub` + `#[doc(hidden)]` solely because the sealed
// `QueryParams` trait's associated types must reference them.  They are not
// part of the public API — fields are private and the trait is sealed so
// external code cannot construct or meaningfully use them.
#[doc(hidden)]
pub trait ComponentStore {
    fn remove_entity(&mut self, id: u32);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

// ---------------------------------------------------------------------------
// SparseSet<T> — per-component storage
// ---------------------------------------------------------------------------

#[doc(hidden)]
pub struct SparseSet<T> {
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
            _marker: PhantomData,
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
// FetchMutEntry — raw-pointer cache for mutable multi-component queries
// ---------------------------------------------------------------------------

/// Caches the raw pointers needed to perform mutable lookups into a
/// [`SparseSet`] without holding a `&mut SparseSet<T>` reference.  This
/// allows multiple component stores to be accessed mutably in parallel.
#[doc(hidden)]
pub struct FetchMutEntry<T> {
    sparse_ptr: *const u32,
    sparse_len: usize,
    dense_ptr: *const u32,
    dense_len: usize,
    data_ptr: *mut T,
    _marker: PhantomData<T>,
}

impl<T: 'static> FetchMutEntry<T> {
    fn from_set(set: &mut SparseSet<T>) -> Self {
        Self {
            sparse_ptr: set.sparse.as_ptr(),
            sparse_len: set.sparse.len(),
            dense_ptr: set.dense.as_ptr(),
            dense_len: set.dense.len(),
            data_ptr: set.data.as_mut_ptr(),
            _marker: PhantomData,
        }
    }

    /// Look up the component for entity `id` and return a mutable reference.
    ///
    /// # Safety
    ///
    /// The caller must guarantee:
    /// - The underlying `SparseSet` has not been moved, reallocated, or
    ///   dropped since this entry was created.
    /// - No other live reference (shared or mutable) exists to the same
    ///   element in `data`.
    /// - Each entity id is fetched at most once across the lifetime of the
    ///   iterator to prevent aliasing mutable references.
    unsafe fn get_mut<'a>(&self, id: u32) -> Option<&'a mut T> {
        let idx = id as usize;
        if idx >= self.sparse_len {
            return None;
        }
        // SAFETY: `idx < sparse_len` guarantees we are in bounds of the
        // original sparse vec.
        let sparse_val = unsafe { *self.sparse_ptr.add(idx) };
        if sparse_val == EMPTY {
            return None;
        }
        // SAFETY: `sparse_val` is a valid dense index (maintained by
        // SparseSet invariants) and the caller ensures no aliasing.
        Some(unsafe { &mut *self.data_ptr.add(sparse_val as usize) })
    }
}

// ---------------------------------------------------------------------------
// Iterators (internal)
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
    _marker: PhantomData<&'a mut T>,
}

impl<'a, T> Iterator for SparseSetIterMut<'a, T> {
    type Item = (u32, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        let i = self.index;
        self.index += 1;
        // SAFETY: This is sound because:
        //  1. `self.data` was obtained from `Vec::as_mut_ptr()` on a live
        //     `SparseSet::data` vec whose buffer is heap-allocated and not
        //     reallocated while this iterator is alive (outlives `'a`).
        //  2. `self.len` equals the vec's length at construction time, so
        //     `i < self.len` guarantees the pointer offset is in bounds.
        //  3. Each index `i` is visited exactly once (monotonically
        //     increasing), so no two iterations yield `&mut` to the same
        //     element — preventing mutable aliasing.
        //  4. The `PhantomData<&'a mut T>` marker ensures the borrow checker
        //     treats this iterator as holding a `&'a mut` borrow over the
        //     entire data buffer, preventing external mutation while the
        //     iterator is alive.
        let val = unsafe { &mut *self.data.add(i) };
        Some((self.dense[i], val))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len - self.index;
        (remaining, Some(remaining))
    }
}

impl<T> ExactSizeIterator for SparseSetIterMut<'_, T> {}

// ---------------------------------------------------------------------------
// Public query iterators (single-component)
// ---------------------------------------------------------------------------

/// Iterator over `(Entity, &T)` pairs from a single-component query.
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

/// Iterator over `(Entity, &mut T)` pairs from a single-component query.
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
// Multi-component query system (sealed trait)
// ---------------------------------------------------------------------------

mod sealed {
    use super::*;

    pub trait Sealed: 'static {
        type Item<'a>;
        type ItemMut<'a>;
        type Fetch<'a>;
        type FetchMut;

        fn init_fetch(
            stores: &HashMap<TypeId, Box<dyn ComponentStore>>,
        ) -> Option<Self::Fetch<'_>>;

        fn fetch_item<'a>(fetch: &Self::Fetch<'a>, id: u32) -> Option<Self::Item<'a>>;

        fn smallest_dense<'a>(fetch: &Self::Fetch<'a>) -> &'a [u32];

        fn init_fetch_mut(
            stores: &mut HashMap<TypeId, Box<dyn ComponentStore>>,
        ) -> Option<(Self::FetchMut, *const u32, usize)>;

        /// # Safety
        ///
        /// The caller must ensure that:
        /// - The raw pointers inside `FetchMut` are still valid.
        /// - Each entity `id` is fetched at most once (no aliasing).
        unsafe fn fetch_item_mut<'a>(
            fetch: &mut Self::FetchMut,
            id: u32,
        ) -> Option<Self::ItemMut<'a>>;
    }
}

/// Trait for multi-component query parameters.
///
/// Implemented for tuples of component types (1 through 8 elements).  This
/// trait is *sealed* — it cannot be implemented outside of this crate.
///
/// Use with [`World::query_multi`] or [`World::query_multi_mut`]:
///
/// ```ignore
/// for (entity, (pos, hp)) in world.query_multi::<(Position, Health)>() { /* … */ }
/// ```
pub trait QueryParams: sealed::Sealed {}
impl<T: sealed::Sealed> QueryParams for T {}

macro_rules! impl_query_params {
    ($($name:ident),+) => {
        #[allow(non_snake_case, unused_assignments)]
        impl<$($name: 'static),+> sealed::Sealed for ($($name,)+) {
            type Item<'a> = ($(&'a $name,)+);
            type ItemMut<'a> = ($(&'a mut $name,)+);
            type Fetch<'a> = ($(&'a SparseSet<$name>,)+);
            type FetchMut = ($(FetchMutEntry<$name>,)+);

            fn init_fetch(
                stores: &HashMap<TypeId, Box<dyn ComponentStore>>,
            ) -> Option<Self::Fetch<'_>> {
                debug_assert!(
                    {
                        let ids = [$(TypeId::of::<$name>(),)+];
                        let len = ids.len();
                        let mut distinct = true;
                        let mut i = 0;
                        while i < len {
                            let mut j = i + 1;
                            while j < len {
                                if ids[i] == ids[j] { distinct = false; }
                                j += 1;
                            }
                            i += 1;
                        }
                        distinct
                    },
                    "query_multi: duplicate component types are likely a bug",
                );
                Some(($(
                    stores
                        .get(&TypeId::of::<$name>())?
                        .as_any()
                        .downcast_ref::<SparseSet<$name>>()?,
                )+))
            }

            fn fetch_item<'a>(
                fetch: &Self::Fetch<'a>,
                id: u32,
            ) -> Option<Self::Item<'a>> {
                let ($($name,)+) = fetch;
                Some(($( $name.get(id)?, )+))
            }

            fn smallest_dense<'a>(fetch: &Self::Fetch<'a>) -> &'a [u32] {
                let ($($name,)+) = fetch;
                let mut smallest: &[u32] = &[];
                let mut smallest_len = usize::MAX;
                $(
                    if $name.dense.len() < smallest_len {
                        smallest_len = $name.dense.len();
                        smallest = &$name.dense;
                    }
                )+
                smallest
            }

            fn init_fetch_mut(
                stores: &mut HashMap<TypeId, Box<dyn ComponentStore>>,
            ) -> Option<(Self::FetchMut, *const u32, usize)> {
                // Distinct types are a *soundness* requirement for mutable
                // queries — duplicate types would alias `&mut` references.
                {
                    let ids = [$(TypeId::of::<$name>(),)+];
                    let len = ids.len();
                    let mut i = 0;
                    while i < len {
                        let mut j = i + 1;
                        while j < len {
                            assert!(
                                ids[i] != ids[j],
                                "query_multi_mut: all component types must be distinct"
                            );
                            j += 1;
                        }
                        i += 1;
                    }
                }

                // Build a FetchMutEntry for each component type. Each
                // `get_mut` borrow is scoped so it ends before the next.
                $(
                    let $name = {
                        let store = stores.get_mut(&TypeId::of::<$name>())?;
                        let set = store
                            .as_any_mut()
                            .downcast_mut::<SparseSet<$name>>()?;
                        FetchMutEntry::from_set(set)
                    };
                )+

                // Find the smallest dense array to drive iteration.
                let mut dense_ptr: *const u32 = std::ptr::null();
                let mut dense_len: usize = 0;
                let mut smallest_len = usize::MAX;
                $(
                    if $name.dense_len < smallest_len {
                        smallest_len = $name.dense_len;
                        dense_ptr = $name.dense_ptr;
                        dense_len = $name.dense_len;
                    }
                )+

                Some((($($name,)+), dense_ptr, dense_len))
            }

            unsafe fn fetch_item_mut<'a>(
                fetch: &mut Self::FetchMut,
                id: u32,
            ) -> Option<Self::ItemMut<'a>> {
                let ($($name,)+) = fetch;
                Some(($( unsafe { $name.get_mut(id)? }, )+))
            }
        }
    };
}

impl_query_params!(A);
impl_query_params!(A, B);
impl_query_params!(A, B, C);
impl_query_params!(A, B, C, D);
impl_query_params!(A, B, C, D, E);
impl_query_params!(A, B, C, D, E, F);
impl_query_params!(A, B, C, D, E, F, G);
impl_query_params!(A, B, C, D, E, F, G, H);

/// Iterator over entities matching an immutable multi-component query.
pub struct QueryParamIter<'a, Q: QueryParams> {
    fetch: Option<<Q as sealed::Sealed>::Fetch<'a>>,
    dense: &'a [u32],
    generations: &'a [u32],
    index: usize,
}

impl<'a, Q: QueryParams> Iterator for QueryParamIter<'a, Q> {
    type Item = (Entity, <Q as sealed::Sealed>::Item<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        let fetch = self.fetch.as_ref()?;
        while self.index < self.dense.len() {
            let id = self.dense[self.index];
            self.index += 1;
            if let Some(item) = <Q as sealed::Sealed>::fetch_item(fetch, id) {
                let entity = Entity {
                    id,
                    generation: self.generations[id as usize],
                };
                return Some((entity, item));
            }
        }
        None
    }
}

/// Iterator over entities matching a mutable multi-component query.
///
/// # Safety (internal)
///
/// This iterator is safe to use from public code because:
///
///  1. [`World::query_multi_mut`] takes `&mut self`, preventing any other
///     access to the world while the iterator is alive.
///  2. `init_fetch_mut` asserts that all component `TypeId`s are distinct,
///     so each [`FetchMutEntry`] points to a different `SparseSet` allocation.
///  3. The iterator visits each entity id at most once (monotonically
///     increasing index), so no two iterations yield `&mut` to the same
///     component instance.
///
/// Note: this type is `!Send` and `!Sync` because it holds raw pointers into
/// the world's component storage.  This is intentional — the iterator is only
/// valid for the duration of the `&mut World` borrow that created it and
/// should not be sent across threads.
pub struct QueryParamIterMut<'a, Q: QueryParams> {
    fetch: Option<<Q as sealed::Sealed>::FetchMut>,
    dense_ptr: *const u32,
    dense_len: usize,
    generations: &'a [u32],
    index: usize,
    _marker: PhantomData<&'a mut ()>,
}

impl<'a, Q: QueryParams> Iterator for QueryParamIterMut<'a, Q> {
    type Item = (Entity, <Q as sealed::Sealed>::ItemMut<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        let fetch = self.fetch.as_mut()?;
        while self.index < self.dense_len {
            // SAFETY: `dense_ptr` was obtained from a valid `SparseSet::dense`
            // vec and `index < dense_len` guarantees we are in bounds.
            let id = unsafe { *self.dense_ptr.add(self.index) };
            self.index += 1;
            // SAFETY: see struct-level safety comment — each id is visited
            // once and all component types are distinct, so no aliasing.
            if let Some(item) =
                unsafe { <Q as sealed::Sealed>::fetch_item_mut(fetch, id) }
            {
                let entity = Entity {
                    id,
                    generation: self.generations[id as usize],
                };
                return Some((entity, item));
            }
        }
        None
    }
}

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
        debug_assert!(
            self.generations[entity.id as usize] < u32::MAX,
            "entity slot {} has been recycled u32::MAX times; generation would \
             wrap and alias a stale Entity handle",
            entity.id,
        );
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

    /// Attach a component to a living entity, replacing any previous value of
    /// the same type.
    ///
    /// # Panics
    ///
    /// Panics if `entity` is dead.  Inserting on a dead entity is always a
    /// programming error — unlike [`get`](Self::get)/[`remove`](Self::remove),
    /// which may encounter a despawned entity as a normal runtime condition
    /// (e.g. another system despawned the entity earlier this frame).
    pub fn insert<T: 'static>(&mut self, entity: Entity, component: T) {
        assert!(
            self.is_alive(entity),
            "cannot insert component on dead entity"
        );
        self.storage_mut::<T>().insert(entity.id, component);
    }

    /// Remove a component from an entity, returning it if present.
    ///
    /// Returns `None` if the entity is dead or does not have a component of
    /// type `T`.  Unlike [`insert`](Self::insert) this does **not** panic on
    /// a dead entity, because encountering a despawned entity is a normal
    /// runtime condition in many ECS patterns.
    pub fn remove<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        if !self.is_alive(entity) {
            return None;
        }
        let store = self.stores.get_mut(&TypeId::of::<T>())?;
        store
            .as_any_mut()
            .downcast_mut::<SparseSet<T>>()?
            .remove(entity.id)
    }

    /// Get a shared reference to an entity's component.
    ///
    /// Returns `None` if the entity is dead or does not have a component of
    /// type `T`.
    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        if !self.is_alive(entity) {
            return None;
        }
        self.storage::<T>()?.get(entity.id)
    }

    /// Get a mutable reference to an entity's component.
    ///
    /// Returns `None` if the entity is dead or does not have a component of
    /// type `T`.
    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        if !self.is_alive(entity) {
            return None;
        }
        let store = self.stores.get_mut(&TypeId::of::<T>())?;
        store
            .as_any_mut()
            .downcast_mut::<SparseSet<T>>()?
            .get_mut(entity.id)
    }

    /// Returns `true` if `entity` is alive and has a component of type `T`.
    pub fn has<T: 'static>(&self, entity: Entity) -> bool {
        if !self.is_alive(entity) {
            return false;
        }
        self.storage::<T>()
            .map_or(false, |s| s.contains(entity.id))
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

    /// Query entities that have *all* of the listed component types (shared).
    ///
    /// The type parameter is a tuple of component types (up to 8). Returns
    /// `(Entity, (&A, &B, ...))` for every entity possessing all components.
    ///
    /// ```ignore
    /// for (entity, (pos, hp)) in world.query_multi::<(Position, Health)>() {
    ///     println!("{:?} at {:?} has {:?} hp", entity, pos, hp);
    /// }
    /// ```
    pub fn query_multi<Q: QueryParams>(&self) -> QueryParamIter<'_, Q> {
        match <Q as sealed::Sealed>::init_fetch(&self.stores) {
            Some(fetch) => {
                let dense = <Q as sealed::Sealed>::smallest_dense(&fetch);
                QueryParamIter {
                    fetch: Some(fetch),
                    dense,
                    generations: &self.allocator.generations,
                    index: 0,
                }
            }
            None => QueryParamIter {
                fetch: None,
                dense: &[],
                generations: &self.allocator.generations,
                index: 0,
            },
        }
    }

    /// Query entities that have *all* of the listed component types (mutable).
    ///
    /// Same as [`query_multi`](Self::query_multi) but yields `&mut` references
    /// to every component in the tuple.
    ///
    /// ```ignore
    /// for (entity, (pos, hp)) in world.query_multi_mut::<(Position, Health)>() {
    ///     pos.x += 1.0;
    ///     hp.0 -= 10;
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the tuple contains duplicate component types (e.g.
    /// `(Health, Health)`), which would create aliasing `&mut` references.
    pub fn query_multi_mut<Q: QueryParams>(&mut self) -> QueryParamIterMut<'_, Q> {
        let generations = &self.allocator.generations;
        match <Q as sealed::Sealed>::init_fetch_mut(&mut self.stores) {
            Some((fetch, dense_ptr, dense_len)) => QueryParamIterMut {
                fetch: Some(fetch),
                dense_ptr,
                dense_len,
                generations,
                index: 0,
                _marker: PhantomData,
            },
            None => QueryParamIterMut {
                fetch: None,
                dense_ptr: std::ptr::null(),
                dense_len: 0,
                generations,
                index: 0,
                _marker: PhantomData,
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
                    _marker: PhantomData,
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
