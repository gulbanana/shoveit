use bevy::{ecs::system::SystemParam, prelude::*, utils::HashSet};
use bevy_rapier2d::prelude::*;

use crate::{Actor, AppState, CacheEvent, InteractionEvent, Tile};

pub const GROUP_ONLY_ALL: Group = Group::from_bits_truncate(1 << 31);
pub const GROUP_WALL: Group = Group::from_bits_truncate(0b0001);
pub const GROUP_ACTOR: Group = Group::from_bits_truncate(0b0010);
pub const GROUP_PIT: Group = Group::from_bits_truncate(0b0100);
pub const GROUP_PIT_WALL: Group = Group::from_bits_truncate(0b1000);

pub const FILTER_ALL: Group = Group::from_bits_truncate(u32::MAX);
pub const FILTER_MAIN: Group = Group::from_bits_truncate(0b0011);
pub const FILTER_PITS: Group = Group::from_bits_truncate(0b0100);
pub const FILTER_WALLS: Group = Group::from_bits_truncate(0b1001);

#[derive(Resource)]
struct ColliderEntities {
    wall_colliders: HashSet<Entity>,
    pit_colliders: HashSet<Entity>,
    actor_colliders: HashSet<Entity>,
}

#[derive(Component)]
struct Intangible {
    frames: u8,
}

#[derive(SystemParam)]
struct Hooks<'w, 's> {
    intangibles: Query<'w, 's, &'static Intangible>,
}

impl BevyPhysicsHooks for Hooks<'_, '_> {
    fn filter_contact_pair(&self, context: PairFilterContextView) -> Option<SolverFlags> {
        if self.intangibles.contains(context.collider1())
            || self.intangibles.contains(context.collider2())
        {
            None
        } else {
            Some(SolverFlags::all())
        }
    }
}

fn setup(mut rapier: ResMut<RapierConfiguration>) {
    rapier.gravity = Vec2::ZERO;
}

fn cache_collider_hierarchy(
    mut cache: ResMut<ColliderEntities>,
    mut input: EventReader<CacheEvent>,
    tiles: Query<(&Children, &Tile)>,
    actors: Query<&Children, With<Actor>>,
) {
    if input
        .iter()
        .map(|event| matches!(event, CacheEvent::InvalidateColliderHierarchy))
        .fold(false, |acc, x| acc || x)
    {
        for (children, tile) in tiles.iter() {
            match tile {
                Tile::Pit => {
                    for child in children.iter() {
                        cache.pit_colliders.insert(*child);
                    }
                }
                Tile::Wall => {
                    for child in children.iter() {
                        cache.wall_colliders.insert(*child);
                    }
                }
            }
        }

        for children in actors.iter() {
            for child in children.iter() {
                cache.actor_colliders.insert(*child);
            }
        }
    }
}

fn detect_collisions(
    cache: Res<ColliderEntities>,
    mut input: EventReader<CollisionEvent>,
    mut output: EventWriter<InteractionEvent>,
    parents: Query<&Parent, With<Collider>>,
) {
    let mut fallen_orbs = HashSet::new();

    let get_parents = |e1: &Entity, e2: &Entity| -> Option<(Entity, Entity)> {
        if let Ok(p1) = parents.get(*e1) {
            if let Ok(p2) = parents.get(*e2) {
                Some((p1.get(), p2.get()))
            } else {
                warn!("unknown parent of collider {e2:?}");
                None
            }
        } else {
            warn!("unknown parent of collider {e1:?}");
            None
        }
    };

    for event in input.iter() {
        if let CollisionEvent::Started(e1, e2, _) = event {
            if cache.pit_colliders.contains(e1) && !fallen_orbs.contains(e2) {
                if let Some((_, p2)) = get_parents(e1, e2) {
                    fallen_orbs.insert(e2);
                    output.send(InteractionEvent::ActorEnterPit(p2));
                }
            } else if cache.pit_colliders.contains(e2) && !fallen_orbs.contains(e1) {
                if let Some((p1, _)) = get_parents(e1, e2) {
                    fallen_orbs.insert(e1);
                    output.send(InteractionEvent::ActorEnterPit(p1));
                }
            } else if (cache.wall_colliders.contains(e1) && cache.actor_colliders.contains(e2))
                || (cache.wall_colliders.contains(e2) && cache.actor_colliders.contains(e1))
            {
                output.send(InteractionEvent::ActorHitWall);
            } else if cache.actor_colliders.contains(e1) && cache.actor_colliders.contains(e2) {
                output.send(InteractionEvent::ActorHitActor);
            } else {
                warn!("unknown collision between {e1:?} and {e2:?}");
            }
        }
    }
}

fn become_tangible(mut commands: Commands, mut query: Query<(Entity, &mut Intangible)>) {
    for (entity, mut intangible) in query.iter_mut() {
        if intangible.frames == 0 {
            commands.entity(entity).remove::<Intangible>();
        } else {
            intangible.frames -= 1;
        }
    }
}

struct CollisionPlugin;

impl Plugin for CollisionPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            RapierPhysicsPlugin::<Hooks>::pixels_per_meter(100.0),
            //RapierDebugRenderPlugin::default(),
        )
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                detect_collisions.before(super::trigger_interaction),
                become_tangible,
            )
                .run_if(in_state(AppState::Playing)),
        )
        .add_systems(PostUpdate, cache_collider_hierarchy)
        .insert_resource(ColliderEntities {
            wall_colliders: HashSet::new(),
            pit_colliders: HashSet::new(),
            actor_colliders: HashSet::new(),
        });
    }
}

pub fn plugin() -> impl Plugin {
    CollisionPlugin
}

// XXX surely there is a builtin version of this
pub struct Rect {
    pub origin: Vec2,
    pub size: Vec2,
}

pub fn spawn_pit(children: &mut ChildBuilder, rect: &Rect) {
    children
        .spawn(SpatialBundle::from_transform(Transform::from_xyz(
            rect.origin.x / 2.0,
            rect.origin.y / 2.0,
            0.0,
        )))
        .insert(Collider::cuboid(rect.size.x / 2.0, rect.size.y / 2.0))
        .insert(CollisionGroups::new(GROUP_PIT, FILTER_ALL))
        .insert(Sensor);
}

pub fn spawn_pit_wall(children: &mut ChildBuilder, rect: &Rect) {
    children
        .spawn(SpatialBundle::from_transform(Transform::from_xyz(
            rect.origin.x / 2.0,
            rect.origin.y / 2.0,
            0.0,
        )))
        .insert(Collider::cuboid(rect.size.x / 2.0, rect.size.y / 2.0))
        .insert(CollisionGroups::new(GROUP_PIT_WALL, FILTER_ALL))
        .insert(Restitution::coefficient(1.0))
        .insert(ActiveHooks::FILTER_CONTACT_PAIRS);
}

pub fn spawn_falling_orb(children: &mut ChildBuilder) {
    children
        .spawn(Collider::ball(100.0))
        .insert(CollisionGroups::new(GROUP_ONLY_ALL, FILTER_WALLS))
        .insert(ColliderMassProperties::Mass(1.0))
        .insert(Restitution::coefficient(1.0))
        .insert(Intangible { frames: 5 });
}
