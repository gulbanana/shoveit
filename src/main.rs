use bevy::prelude::*;
use bevy::utils::HashSet;
use bevy::{math::Vec3Swizzles, render::camera::ScalingMode};
use bevy_ecs_ldtk::prelude::*;
use bevy_rapier2d::prelude::*;
use std::f32::consts::PI;

mod loader;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States)]
enum AppState {
    #[default]
    Loading,
    Playing,
}

#[derive(Event)]
enum DetectionEvent {
    OrbWall,
    OrbOrb,
    OrbPit(Entity),
}

#[derive(Default, Component)]
struct WallTile;

#[derive(Default, Component)]
struct PitTile;

#[derive(Default, Component)]
struct Orb;

#[derive(Default, Component)]
struct Player;

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
            loader::LoaderPlugin,
        ))
        .add_state::<AppState>()
        .add_event::<DetectionEvent>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                move_player,
                cap_player_velocity,
                detect_collisions,
                on_collision.after(detect_collisions),
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
        //eprintln!("angle_between: {}", transform.rotation.angle_between(arc));
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
    mut output: EventWriter<DetectionEvent>,
    pit_query: Query<Entity, With<PitTile>>,
    wall_query: Query<Entity, With<WallTile>>,
) {
    let mut pit_entities = HashSet::new();
    for entity in pit_query.iter() {
        pit_entities.insert(entity);
    }

    let mut wall_entities = HashSet::new();
    for entity in wall_query.iter() {
        wall_entities.insert(entity);
    }

    let mut fallen_orbs = HashSet::new();

    for event in input.iter() {
        if let CollisionEvent::Started(e1, e2, _) = event {
            if pit_entities.contains(e1) && !fallen_orbs.contains(e2) {
                fallen_orbs.insert(e2);
                output.send(DetectionEvent::OrbPit(*e2));
            } else if pit_entities.contains(e2) && !fallen_orbs.contains(e1) {
                fallen_orbs.insert(e1);
                output.send(DetectionEvent::OrbPit(*e1));
            } else if wall_entities.contains(e1) || wall_entities.contains(e2) {
                output.send(DetectionEvent::OrbWall);
            } else {
                output.send(DetectionEvent::OrbOrb);
            }
        }
    }
}

fn on_collision(
    assets: Res<AssetServer>,
    mut commands: Commands,
    mut events: EventReader<DetectionEvent>,
    players: Query<Entity, With<Player>>,
) {
    for event in events.iter() {
        match event {
            DetectionEvent::OrbWall => {
                commands.spawn(AudioBundle {
                    source: assets.load("pobble.ogg"),
                    ..default()
                });
            }
            DetectionEvent::OrbOrb => {
                commands.spawn(AudioBundle {
                    source: assets.load("pobblebonk.ogg"),
                    ..default()
                });
            }
            DetectionEvent::OrbPit(entity) => {
                let all_players = HashSet::from_iter(players.iter());
                commands.spawn(AudioBundle {
                    source: assets.load(if all_players.contains(entity) {
                        "player-fall.ogg"
                    } else {
                        "enemy-fall.ogg"
                    }),
                    ..default()
                });

                commands.entity(*entity).despawn();
            }
        }
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
        match level.into_inner() {
            LevelSelection::Index(i) => {
                commands.insert_resource(LevelSelection::Index(1 - i));
                next_state.set(AppState::Loading);
            }
            _ => (),
        }
    }
}
