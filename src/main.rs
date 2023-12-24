use bevy::{prelude::*, render::camera::ScalingMode};
use bevy_ecs_ldtk::prelude::*;
use bevy_rapier2d::prelude::*;

#[derive(Bundle)]
struct ControlledBundle {
    rigid_body: RigidBody,
    mass: AdditionalMassProperties,
    velocity: Velocity,
    thrust: ExternalImpulse,
}

impl Default for ControlledBundle {
    fn default() -> Self {
        Self {
            rigid_body: RigidBody::Dynamic,
            mass: AdditionalMassProperties::Mass(1.0),
            velocity: Velocity::default(),
            thrust: ExternalImpulse::default(),
        }
    }
}

#[derive(Bundle, LdtkEntity)]
struct PlayerBundle {
    #[from_entity_instance]
    instance: EntityInstance,
    #[sprite_sheet_bundle]
    sprite_bundle: SpriteSheetBundle,
    controlled_bundle: ControlledBundle,
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
            RapierDebugRenderPlugin::default(),
        ))
        .add_systems(Startup, startup)
        .add_systems(Update, (move_player, orient_player, cap_velocity))
        .insert_resource(LevelSelection::Index(0))
        .register_ldtk_entity::<PlayerBundle>("Player")
        .run();
}

fn startup(
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

fn move_player(
    mut query: Query<&mut ExternalImpulse, With<EntityInstance>>,
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

fn orient_player(mut query: Query<(&Velocity, &mut Transform), With<EntityInstance>>) {
    for (velocity, mut transform) in query.iter_mut() {
        transform.rotation =
            Quat::from_rotation_arc_2d(Vec2::new(0.0, 1.0), velocity.linvel.normalize());
    }
}

fn cap_velocity(mut query: Query<&mut Velocity, With<EntityInstance>>) {
    for mut velocity in query.iter_mut() {
        velocity.linvel = velocity.linvel.clamp_length_max(1250.0);
    }
}
