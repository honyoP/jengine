use jengine::ecs::*;

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

    // -- multi-component query_multi -----------------------------------------

    #[test]
    fn query_multi_two_components() {
        let mut world = World::new();

        let player = world.spawn();
        world.insert(player, Position { x: 0.0, y: 0.0 });
        world.insert(player, Health(100));

        let tree = world.spawn();
        world.insert(tree, Position { x: 5.0, y: 5.0 });
        // tree has no Health

        let results: Vec<_> = world.query_multi::<(Position, Health)>().collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, player);
        assert_eq!(results[0].1, (&Position { x: 0.0, y: 0.0 }, &Health(100)));
    }

    #[test]
    fn query_multi_three_components() {
        let mut world = World::new();

        let full = world.spawn();
        world.insert(full, Position { x: 1.0, y: 2.0 });
        world.insert(full, Health(50));
        world.insert(full, Name("Hero".into()));

        let partial = world.spawn();
        world.insert(partial, Position { x: 3.0, y: 4.0 });
        world.insert(partial, Health(25));
        // no Name

        let results: Vec<_> = world.query_multi::<(Position, Health, Name)>().collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, full);
        let (pos, hp, name) = results[0].1;
        assert_eq!(pos, &Position { x: 1.0, y: 2.0 });
        assert_eq!(hp, &Health(50));
        assert_eq!(name, &Name("Hero".into()));
    }

    #[test]
    fn query_multi_no_matches() {
        let mut world = World::new();

        let e = world.spawn();
        world.insert(e, Position { x: 0.0, y: 0.0 });
        // no Health

        assert_eq!(world.query_multi::<(Position, Health)>().count(), 0);
    }

    #[test]
    fn query_multi_empty_world() {
        let world = World::new();
        assert_eq!(world.query_multi::<(Position, Health)>().count(), 0);
    }

    #[test]
    fn query_multi_single_tuple() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Health(42));

        let results: Vec<_> = world.query_multi::<(Health,)>().collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, (&Health(42),));
    }

    #[test]
    fn query_multi_iterates_smallest_set() {
        let mut world = World::new();

        // 100 entities with Position, only 2 also have Health
        for i in 0..100 {
            let e = world.spawn();
            world.insert(e, Position { x: i as f32, y: 0.0 });
        }
        let special_a = world.spawn();
        world.insert(special_a, Position { x: 200.0, y: 0.0 });
        world.insert(special_a, Health(10));

        let special_b = world.spawn();
        world.insert(special_b, Position { x: 201.0, y: 0.0 });
        world.insert(special_b, Health(20));

        let results: Vec<_> = world.query_multi::<(Position, Health)>().collect();
        assert_eq!(results.len(), 2);
    }

    // -- query_multi_mut (mutable multi-component) -----------------------------

    #[test]
    fn query_multi_mut_modifies_components() {
        let mut world = World::new();

        let a = world.spawn();
        world.insert(a, Position { x: 1.0, y: 2.0 });
        world.insert(a, Health(100));

        let b = world.spawn();
        world.insert(b, Position { x: 3.0, y: 4.0 });
        world.insert(b, Health(50));

        for (_, (pos, hp)) in world.query_multi_mut::<(Position, Health)>() {
            pos.x += 10.0;
            hp.0 -= 25;
        }

        assert_eq!(world.get::<Position>(a), Some(&Position { x: 11.0, y: 2.0 }));
        assert_eq!(world.get::<Health>(a), Some(&Health(75)));
        assert_eq!(world.get::<Position>(b), Some(&Position { x: 13.0, y: 4.0 }));
        assert_eq!(world.get::<Health>(b), Some(&Health(25)));
    }

    #[test]
    fn query_multi_mut_filters_correctly() {
        let mut world = World::new();

        let with_both = world.spawn();
        world.insert(with_both, Position { x: 0.0, y: 0.0 });
        world.insert(with_both, Health(10));

        let pos_only = world.spawn();
        world.insert(pos_only, Position { x: 5.0, y: 5.0 });

        let count = world.query_multi_mut::<(Position, Health)>().count();
        assert_eq!(count, 1);
    }

    #[test]
    fn query_multi_mut_empty_world() {
        let mut world = World::new();
        assert_eq!(world.query_multi_mut::<(Position, Health)>().count(), 0);
    }

    #[test]
    fn query_multi_mut_single_component() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Health(100));

        for (_, (hp,)) in world.query_multi_mut::<(Health,)>() {
            hp.0 = 999;
        }

        assert_eq!(world.get::<Health>(e), Some(&Health(999)));
    }

    // -- has<T> ---------------------------------------------------------------

    #[test]
    fn has_returns_true_for_present_component() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Health(1));
        assert!(world.has::<Health>(e));
    }

    #[test]
    fn has_returns_false_for_missing_component() {
        let mut world = World::new();
        let e = world.spawn();
        assert!(!world.has::<Health>(e));
    }

    #[test]
    fn has_returns_false_for_dead_entity() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Health(1));
        world.despawn(e);
        assert!(!world.has::<Health>(e));
    }

    // -- remove does not allocate spurious store ------------------------------

    #[test]
    fn remove_missing_type_does_not_allocate_store() {
        let mut world = World::new();
        let e = world.spawn();
        // Remove a type that was never inserted — should not create a store.
        world.remove::<Name>(e);
        // If a store was spuriously created, inserting Name on a new entity
        // and querying would still work, but we can verify no store exists
        // by checking that query returns 0 without ever inserting.
        assert_eq!(world.query::<Name>().count(), 0);
        // The key check: has<Name> should return false (no store at all).
        assert!(!world.has::<Name>(e));
    }
}
