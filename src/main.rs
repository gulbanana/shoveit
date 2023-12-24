use bevy::{prelude::*, render::camera::ScalingMode};
use bevy_ecs_ldtk::prelude::*;
use std::f32::consts::PI;

#[derive(Bundle, LdtkEntity)]
struct PlayerBundle {
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
        ))
        .add_systems(Startup, startup)
        .add_systems(Update, move_player)
        .insert_resource(LevelSelection::Index(0))
        .register_ldtk_entity::<PlayerBundle>("Player")
        .run();
}

fn startup(mut commands: Commands, asset_server: Res<AssetServer>) {
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
}

fn move_player(
    mut player: Query<(&EntityInstance, &mut Transform)>,
    time: Res<Time>,
    input: Res<Input<KeyCode>>,
) {
    let mut arc = 0f32;

    if input.pressed(KeyCode::Right) {
        arc -= 2.0 * PI * time.delta_seconds();
    }

    if input.pressed(KeyCode::Left) {
        arc += 2.0 * PI * time.delta_seconds();
    }

    for (_, mut transform) in player.iter_mut() {
        transform.rotate(Quat::from_rotation_z(arc));
    }
}
