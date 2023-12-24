use bevy::{prelude::*, render::camera::ScalingMode};
use bevy_ecs_ldtk::prelude::*;
use bevy_rapier2d::prelude::*;

#[derive(Default, Component)]
struct WallTile;

#[derive(Bundle, LdtkIntCell)]
struct TileBundle {
    marker: WallTile,
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
        .add_systems(Startup, setup_game)
        .add_systems(
            Update,
            (
                setup_level,
                setup_player,
                move_player,
                orient_player,
                cap_velocity,
            ),
        )
        .insert_resource(LevelSelection::Index(0))
        .register_ldtk_entity::<PlayerBundle>("player")
        .register_ldtk_int_cell::<TileBundle>(1)
        .run();
}

fn setup_game(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut rapier: ResMut<RapierConfiguration>,
) {
    rapier.gravity = Vec2::ZERO;

    let bounds = Vec3::new(4096.0, 2304.0, 0.0);
    let origin = bounds / 2.0;

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

    commands.spawn(RigidBody::Dynamic);
}

fn setup_level(mut commands: Commands, mut query: Query<Entity, Added<WallTile>>) {
    for id in query.iter_mut() {
        commands
            .entity(id)
            .insert(Collider::cuboid(128.0, 128.0))
            .insert(Restitution::coefficient(1.0));
    }
}

fn setup_player(mut commands: Commands, mut query: Query<Entity, Added<Player>>) {
    for id in query.iter_mut() {
        commands
            .entity(id)
            .insert(RigidBody::Dynamic)
            .insert(Collider::ball(100.0))
            .insert(ColliderMassProperties::Mass(1.0))
            .insert(Restitution::coefficient(1.0))
            .insert(Velocity::default())
            .insert(ExternalImpulse::default())
            .insert(LockedAxes::ROTATION_LOCKED);
    }
}

fn move_player(
    mut query: Query<&mut ExternalImpulse, With<Player>>,
    time: Res<Time>,
    input: Res<Input<KeyCode>>,
) {
    let mut accel = Vec2::ZERO;

    if input.pressed(KeyCode::Right) {
        accel.x += 500.0 * time.delta_seconds();
    }

    if input.pressed(KeyCode::Left) {
        accel.x -= 500.0 * time.delta_seconds();
    }

    if input.pressed(KeyCode::Up) {
        accel.y += 500.0 * time.delta_seconds();
    }

    if input.pressed(KeyCode::Down) {
        accel.y -= 500.0 * time.delta_seconds();
    }

    for mut thrust in query.iter_mut() {
        thrust.impulse = accel;
    }
}

fn orient_player(mut query: Query<(&Velocity, &mut Transform), With<Player>>) {
    for (velocity, mut transform) in query.iter_mut() {
        if velocity.linvel.length() > 0.0 {
            transform.rotation =
                Quat::from_rotation_arc_2d(Vec2::new(0.0, 1.0), velocity.linvel.normalize());
        }
    }
}

fn cap_velocity(mut query: Query<&mut Velocity, With<Player>>) {
    for mut velocity in query.iter_mut() {
        velocity.linvel = velocity.linvel.clamp_length_max(2500.0);
    }
}
