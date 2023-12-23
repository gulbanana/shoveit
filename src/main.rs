use bevy::{prelude::*, render::camera::ScalingMode};
use bevy_ecs_ldtk::prelude::*;

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
        .insert_resource(LevelSelection::Index(0))
        .run();
}

fn startup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let bounds = Vec3::new(4096.0, 2304.0, 0.0);
    let origin = bounds / 2.0;

    commands.spawn(Camera2dBundle {
        transform: Transform::from_translation(origin),
        projection: OrthographicProjection {
            scaling_mode: ScalingMode::FixedHorizontal(bounds.x),
            ..default()
        },
        ..default()
    });

    commands.spawn(LdtkWorldBundle {
        ldtk_handle: asset_server.load("levels.ldtk"),
        ..default()
    });
}
