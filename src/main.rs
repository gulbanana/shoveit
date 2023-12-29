use bevy::prelude::*;
use bevy::utils::{HashMap, HashSet};
use bevy::{math::Vec3Swizzles, render::camera::ScalingMode};
use bevy_rapier2d::prelude::*;
use bevy_tweening::lens::TransformScaleLens;
use bevy_tweening::*;
use std::f32::consts::PI;
use std::time::Duration;

mod collision;
mod level;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States)]
enum AppState {
    #[default]
    Loading,
    Playing,
}

/// Player button presses
#[derive(Event)]
enum InputEvent {
    Decelerate,
    Accelerate(Vec2),
}

/// Interactions detected by physics
#[derive(Event)]
enum InteractionEvent {
    ActorHitActor,
    ActorHitWall,
    ActorEnterPit { actor: Entity, pit: Entity },
}

/// Has interactions on contact
#[derive(Component)]
enum Tile {
    Wall,
    Pit,
}

/// Moves around the level, interacting with other actors and with tiles
#[derive(Component)]
struct Actor {
    sfx: String,
}

#[derive(Component, Default)]
struct PlayerControl;

#[derive(Component)]
enum EnemyControl {
    Cowardice,
    Malice,
}

#[derive(Resource)]
struct AnimationCompletions {
    next: u64,
    killers: HashMap<u64, Entity>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut level = 0;
    if let Some(arg) = args.get(1) {
        if let Ok(index) = arg.parse() {
            level = index;
        }
    }

    App::new()
        .add_plugins((
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Shove it!".into(),
                        ..default()
                    }),
                    ..default()
                }),
            RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0),
            //RapierDebugRenderPlugin::default(),
            TweeningPlugin,
            level::LoaderPlugin { level },
        ))
        .add_state::<AppState>()
        .add_event::<InputEvent>()
        .add_event::<InteractionEvent>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                keyboard_input.before(move_player),
                move_player.before(cap_player_velocity),
                cap_player_velocity,
                detect_collisions,
                trigger_interaction.after(detect_collisions),
                die_after_fall,
            )
                .run_if(in_state(AppState::Playing)),
        )
        .insert_resource(AnimationCompletions {
            next: 0,
            killers: HashMap::new(),
        })
        .run();
}

fn handle(result: In<anyhow::Result<()>>) {
    if let In(Result::Err(cause)) = result {
        error!("{}", cause);
    }
}

fn setup(mut commands: Commands, mut rapier: ResMut<RapierConfiguration>) {
    rapier.gravity = Vec2::ZERO;

    let bounds = Vec3::new(4096.0, 2304.0, 0.0);
    let offset = Vec3::new(512.0, 512.0, 0.0); // 2-tile border for ratio safety
    let origin = bounds / 2.0 + offset;

    commands.spawn(Camera2dBundle {
        transform: Transform::from_translation(origin),
        projection: OrthographicProjection {
            far: 1000.0,
            near: -1000.0,
            scaling_mode: ScalingMode::FixedHorizontal(4096.0),
            ..default()
        },
        ..default()
    });
}

fn keyboard_input(input: Res<Input<KeyCode>>, mut events: EventWriter<InputEvent>) {
    // braking takes priority
    if input.pressed(KeyCode::Space) {
        events.send(InputEvent::Decelerate);
        return;
    }

    // if not braking, we may thrust
    let mut thrust = Vec2::ZERO;

    if input.pressed(KeyCode::Right) {
        thrust.x += 1.0;
    }

    if input.pressed(KeyCode::Left) {
        thrust.x -= 1.0;
    }

    if input.pressed(KeyCode::Up) {
        thrust.y += 1.0;
    }

    if input.pressed(KeyCode::Down) {
        thrust.y -= 1.0;
    }

    if thrust != Vec2::ZERO {
        thrust = thrust.normalize();
        events.send(InputEvent::Accelerate(thrust));
    }
}

fn move_player(
    time: Res<Time>,
    mut events: EventReader<InputEvent>,
    mut query: Query<(&mut Transform, &mut Velocity, &mut ExternalImpulse), With<PlayerControl>>,
) {
    for event in events.iter() {
        match *event {
            InputEvent::Decelerate => {
                for (_, velocity, mut impulse) in query.iter_mut() {
                    let antithrust = velocity.linvel.normalize();
                    impulse.impulse = (antithrust * -1500.0 * time.delta_seconds())
                        .clamp_length(0.0, velocity.linvel.length());
                }
            }
            InputEvent::Accelerate(thrust) => {
                for (mut transform, mut velocity, mut impulse) in query.iter_mut() {
                    let forward = (transform.rotation * Vec3::Y).xy();
                    let forward_dot_goal = forward.dot(thrust);

                    // if facing ⋅ thrust is significant, rotate towards thrust
                    if (forward_dot_goal - 1.0).abs() >= f32::EPSILON {
                        // cancel any tumbling
                        velocity.angvel = 0.0;

                        // +ve=anticlockwise, -ve=clockwise (right hand rule)
                        let right = (transform.rotation * Vec3::X).xy();
                        let right_dot_goal = right.dot(thrust);
                        let sign = -f32::copysign(1.0, right_dot_goal);

                        // avoid overshoot
                        let max_angle = forward_dot_goal.clamp(-1.0, 1.0).acos();
                        let rotation_angle =
                            (sign * 4.0 * PI * time.delta_seconds()).min(max_angle);

                        transform.rotate_z(rotation_angle);
                    }
                    // otherwise, apply thrust in the direction we are now facing
                    else {
                        impulse.impulse = thrust * 750.0 * time.delta_seconds();
                    }
                }
            }
        }
    }
}

fn cap_player_velocity(mut query: Query<&mut Velocity, With<PlayerControl>>) {
    for mut velocity in query.iter_mut() {
        velocity.linvel = velocity.linvel.clamp_length_max(3000.0);
    }
}

fn detect_collisions(
    mut input: EventReader<CollisionEvent>,
    mut output: EventWriter<InteractionEvent>,
    parents: Query<&Parent, With<Collider>>,
    tiles: Query<(&Tile, &Children)>,
    actors: Query<&Children, With<Actor>>,
) {
    // XXX build these structures only when the set of actors/tiles/colliders changes
    let mut pit_colliders = HashSet::new();
    let mut wall_colliders = HashSet::new();
    let mut actor_colliders = HashSet::new();

    for (tile, children) in tiles.iter() {
        match tile {
            Tile::Pit => {
                for child in children.iter() {
                    pit_colliders.insert(child);
                }
            }
            Tile::Wall => {
                for child in children.iter() {
                    wall_colliders.insert(child);
                }
            }
        }
    }

    for children in actors.iter() {
        for child in children.iter() {
            actor_colliders.insert(child);
        }
    }

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
            if pit_colliders.contains(e1) && !fallen_orbs.contains(e2) {
                if let Some((p1, p2)) = get_parents(e1, e2) {
                    fallen_orbs.insert(e2);
                    output.send(InteractionEvent::ActorEnterPit { actor: p2, pit: p1 });
                }
            } else if pit_colliders.contains(e2) && !fallen_orbs.contains(e1) {
                if let Some((p1, p2)) = get_parents(e1, e2) {
                    fallen_orbs.insert(e1);
                    output.send(InteractionEvent::ActorEnterPit { actor: p1, pit: p2 });
                }
            } else if (wall_colliders.contains(e1) && actor_colliders.contains(e2))
                || (wall_colliders.contains(e2) && actor_colliders.contains(e1))
            {
                output.send(InteractionEvent::ActorHitWall);
            } else if actor_colliders.contains(e1) && actor_colliders.contains(e2) {
                output.send(InteractionEvent::ActorHitActor);
            } else {
                warn!("unknown collision between {e1:?} and {e2:?}");
            }
        }
    }
}

fn trigger_interaction(
    assets: Res<AssetServer>,
    mut completions: ResMut<AnimationCompletions>,
    mut commands: Commands,
    mut events: EventReader<InteractionEvent>,
    actors: Query<&Actor>,
) {
    for event in events.iter() {
        match event {
            InteractionEvent::ActorHitWall => {
                commands.spawn(AudioBundle {
                    source: assets.load("pobble.ogg"),
                    ..default()
                });
            }
            InteractionEvent::ActorHitActor => {
                commands.spawn(AudioBundle {
                    source: assets.load("pobblebonk.ogg"),
                    ..default()
                });
            }
            InteractionEvent::ActorEnterPit { actor, pit } => {
                if let Ok(actor) = actors.get(*actor) {
                    commands.spawn(AudioBundle {
                        source: assets.load(&actor.sfx),
                        ..default()
                    });
                }

                // shrink into oblivion
                let animation_id = completions.next;
                let tween = Tween::new(
                    EaseFunction::QuadraticIn,
                    Duration::from_millis(1500),
                    TransformScaleLens {
                        start: Vec3::ONE,
                        end: Vec3::ZERO,
                    },
                )
                .with_completed_event(animation_id);

                commands
                    .entity(*actor)
                    .remove::<Actor>()
                    .insert(Animator::new(tween))
                    .despawn_descendants()
                    .with_children(|children| {
                        children
                            .spawn(Collider::ball(100.0))
                            .insert(CollisionGroups::new(
                                collision::GROUP_ONLY_ALL,
                                collision::FILTER_WALLS,
                            ))
                            .insert(ColliderMassProperties::Mass(1.0))
                            .insert(Restitution::coefficient(1.0));
                    });

                commands.entity(*pit).insert(RigidBodyDisabled);
                completions.killers.insert(animation_id, *pit);
                completions.next += 1;
            }
        }
    }
}

fn die_after_fall(
    mut completions: ResMut<AnimationCompletions>,
    mut commands: Commands,
    mut events: EventReader<TweenCompleted>,
    query: Query<Entity, (With<Tile>, With<RigidBodyDisabled>)>,
) {
    for fallen in events.iter() {
        commands.entity(fallen.entity).despawn_recursive();
        if let Some(killer) = completions.killers.get(&fallen.user_data) {
            if let Ok(pit) = query.get(*killer) {
                commands.entity(pit).remove::<RigidBodyDisabled>();
            }
            completions.killers.remove(&fallen.user_data);
        } else {
            warn!("no killer registered for completion {}", fallen.user_data)
        }
    }
}
