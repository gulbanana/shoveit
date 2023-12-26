use std::f32::consts::PI;

use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy::render::camera::ScalingMode;
use bevy_ecs_ldtk::prelude::*;
use bevy_rapier2d::prelude::*;

#[derive(Default, Component)]
struct WallTile;

#[derive(Bundle, LdtkIntCell)]
struct WallBundle {
    marker: WallTile,
}

#[derive(Default, Component)]
struct PitTile;

#[derive(Bundle, LdtkIntCell)]
struct PitBundle {
    marker: PitTile,
}

#[derive(Default, Component)]
struct Player;

#[derive(Bundle)]
struct ControlledBundle {
    rigid_body: RigidBody,
    velocity: Velocity,
    thrust: ExternalImpulse,
    collider: Collider,
    mass: ColliderMassProperties,
    restitution: Restitution,
}

#[derive(Bundle, LdtkEntity)]
struct PlayerBundle {
    player: Player,
    #[from_entity_instance]
    instance: EntityInstance,
    #[sprite_sheet_bundle]
    sprite_bundle: SpriteSheetBundle,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
enum AppState {
    #[default]
    Loading,
    Playing,
}

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
            LdtkPlugin,
            RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0),
            //RapierDebugRenderPlugin::default(),
        ))
        .add_state::<AppState>()
        .add_systems(Startup, setup_game)
        .add_systems(
            Update,
            (
                setup_walls,
                setup_pits,
                setup_player,
                detect_loading_complete,
            )
                .run_if(in_state(AppState::Loading)),
        )
        .add_systems(
            Update,
            (
                move_player,
                cap_player_velocity,
                fall_into_pit,
                respawn_after_death,
            )
                .run_if(in_state(AppState::Playing)),
        )
        .register_ldtk_entity::<PlayerBundle>("player")
        .register_ldtk_int_cell::<WallBundle>(1)
        .register_ldtk_int_cell::<PitBundle>(2)
        .run();
}

fn setup_game(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut rapier: ResMut<RapierConfiguration>,
) {
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

    commands.spawn(LdtkWorldBundle {
        ldtk_handle: asset_server.load("levels.ldtk"),
        ..default()
    });

    commands.insert_resource(LevelSelection::Index(0))
}

fn setup_walls(mut commands: Commands, mut query: Query<Entity, Added<WallTile>>) {
    for id in query.iter_mut() {
        commands
            .entity(id)
            .insert(Collider::cuboid(128.0, 128.0))
            .insert(Restitution::coefficient(1.0));
    }
}

fn setup_pits(mut commands: Commands, mut query: Query<Entity, Added<PitTile>>) {
    for id in query.iter_mut() {
        commands
            .entity(id)
            .insert(Collider::cuboid(64.0, 64.0))
            .insert(Sensor)
            .insert(ActiveEvents::COLLISION_EVENTS);
    }
}

fn setup_player(mut commands: Commands, mut query: Query<Entity, Added<Player>>) {
    for id in query.iter_mut() {
        commands
            .entity(id)
            .insert(RigidBody::Dynamic)
            .insert(Velocity::default())
            .insert(ExternalImpulse::default())
            .insert(Collider::ball(100.0))
            .insert(ColliderMassProperties::Mass(1.0))
            .insert(Restitution::coefficient(1.0));
    }
}

fn detect_loading_complete(
    mut events: EventReader<LevelEvent>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for level_event in events.iter() {
        match level_event {
            LevelEvent::Transformed(_iid) => next_state.set(AppState::Playing),
            _ => (),
        }
    }
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

fn fall_into_pit(
    mut commands: Commands,
    mut events: EventReader<CollisionEvent>,
    query: Query<Entity, With<Player>>,
) {
    for collision_event in events.iter() {
        if let CollisionEvent::Started(_, _, _) = collision_event {
            commands.entity(query.single()).despawn();
        }
    }
}

fn respawn_after_death(
    mut commands: Commands,
    level: Res<LevelSelection>,
    mut next_state: ResMut<NextState<AppState>>,
    query: Query<&Player>,
) {
    if query.is_empty() {
        match level.into_inner() {
            LevelSelection::Index(i) => {
                commands.insert_resource(LevelSelection::Index(1 - i));
                next_state.set(AppState::Loading);
            }
            _ => (),
        }
    }
}
