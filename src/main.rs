use bevy::prelude::*;
use bevy::utils::HashSet;
use bevy::{math::Vec3Swizzles, render::camera::ScalingMode};
use bevy_ecs_ldtk::prelude::*;
use bevy_rapier2d::prelude::*;
use bevy_tweening::lens::TransformScaleLens;
use bevy_tweening::*;
use std::f32::consts::PI;
use std::time::Duration;

mod loader;

const ANIMATION_FALL: u64 = 0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States)]
enum AppState {
    #[default]
    Loading,
    Playing,
}

/// Interactions detected by physics
#[derive(Event)]
enum InteractionEvent {
    ActorHitActor,
    ActorHitWall,
    ActorEnterPit(Entity),
}

/// Has interactions on contact
#[derive(Component)]
enum Tile {
    Wall,
    Pit,
}

/// Moves around the level, interacting with other actors and with tiles
#[derive(Default, Component)]
struct Actor;

/// Marks pc, who must remain alive
#[derive(Default, Component)]
struct Player;

/// Marks npc, who can be defeated
#[derive(Default, Component)]
struct Enemy;

fn main() {
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
            loader::LoaderPlugin,
        ))
        .add_state::<AppState>()
        .add_event::<InteractionEvent>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                move_player,
                cap_player_velocity,
                detect_collisions,
                trigger_interaction.after(detect_collisions),
                die_after_fall,
                respawn_after_death,
                advance_after_victory,
            )
                .run_if(in_state(AppState::Playing)),
        )
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

fn move_player(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Velocity, &mut ExternalImpulse), With<Player>>,
    input: Res<Input<KeyCode>>,
) {
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

    if thrust == Vec2::ZERO {
        return;
    } else {
        thrust = thrust.normalize();
    }

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
            let rotation_angle = (sign * 4.0 * PI * time.delta_seconds()).min(max_angle);

            transform.rotate_z(rotation_angle);
        }
        // otherwise, apply thrust in the direction we are now facing
        else {
            impulse.impulse = thrust * 750.0 * time.delta_seconds();
        }
    }
}

fn cap_player_velocity(mut query: Query<&mut Velocity, With<Player>>) {
    for mut velocity in query.iter_mut() {
        velocity.linvel = velocity.linvel.clamp_length_max(3000.0);
    }
}

fn detect_collisions(
    mut input: EventReader<CollisionEvent>,
    mut output: EventWriter<InteractionEvent>,
    query: Query<(Entity, &Tile)>,
) {
    let mut pit_entities = HashSet::new();
    let mut wall_entities = HashSet::new();

    for (entity, tile) in query.iter() {
        match tile {
            Tile::Pit => {
                pit_entities.insert(entity);
            }
            Tile::Wall => {
                wall_entities.insert(entity);
            }
        }
    }

    let mut fallen_orbs = HashSet::new();

    for event in input.iter() {
        if let CollisionEvent::Started(e1, e2, _) = event {
            if pit_entities.contains(e1) && !fallen_orbs.contains(e2) {
                fallen_orbs.insert(e2);
                output.send(InteractionEvent::ActorEnterPit(*e2));
            } else if pit_entities.contains(e2) && !fallen_orbs.contains(e1) {
                fallen_orbs.insert(e1);
                output.send(InteractionEvent::ActorEnterPit(*e1));
            } else if wall_entities.contains(e1) || wall_entities.contains(e2) {
                output.send(InteractionEvent::ActorHitWall);
            } else {
                output.send(InteractionEvent::ActorHitActor);
            }
        }
    }
}

fn trigger_interaction(
    assets: Res<AssetServer>,
    mut commands: Commands,
    mut events: EventReader<InteractionEvent>,
    players: Query<Entity, With<Player>>,
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
            InteractionEvent::ActorEnterPit(entity) => {
                let all_players = HashSet::from_iter(players.iter());
                commands.spawn(AudioBundle {
                    source: assets.load(if all_players.contains(entity) {
                        "player-fall.ogg"
                    } else {
                        "enemy-fall.ogg"
                    }),
                    ..default()
                });

                // shrink into oblivion
                let tween = Tween::new(
                    EaseFunction::QuadraticIn,
                    Duration::from_secs(1),
                    TransformScaleLens {
                        start: Vec3::ONE,
                        end: Vec3::ZERO,
                    },
                )
                .with_completed_event(ANIMATION_FALL);

                commands
                    .entity(*entity)
                    .remove::<Actor>()
                    .remove::<Collider>()
                    .insert(Animator::new(tween));
            }
        }
    }
}

fn die_after_fall(mut commands: Commands, mut events: EventReader<TweenCompleted>) {
    for fallen in events.iter() {
        commands.entity(fallen.entity).despawn();
    }
}

fn respawn_after_death(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    level: Query<Entity, With<Handle<LdtkLevel>>>,
    players: Query<&Player>,
) {
    if players.is_empty() {
        commands.entity(level.single()).insert(Respawn);
        next_state.set(AppState::Loading);
    }
}

fn advance_after_victory(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    level: Res<LevelSelection>,
    enemies: Query<&Enemy>,
) {
    if enemies.is_empty() {
        if let LevelSelection::Index(i) = level.into_inner() {
            commands.insert_resource(LevelSelection::Index(1 - i));
            next_state.set(AppState::Loading);
        }
    }
}
