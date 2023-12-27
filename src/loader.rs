use super::AppState;
use anyhow::Context;
use bevy::{prelude::*, utils::HashMap};
use bevy_ecs_ldtk::prelude::*;
use bevy_rapier2d::prelude::*;
use serde::Deserialize;

const WALL_TILE: i32 = 1;
const PIT_TILE: i32 = 2;

#[derive(Deserialize, Debug)]
struct CustomData {
    insets: [f32; 4],
}

impl CustomData {
    fn inset_top(&self) -> f32 {
        self.insets[0]
    }

    fn inset_right(&self) -> f32 {
        self.insets[1]
    }

    fn inset_bottom(&self) -> f32 {
        self.insets[2]
    }

    fn inset_left(&self) -> f32 {
        self.insets[3]
    }
}

#[derive(Bundle, LdtkEntity)]
struct PlayerBundle {
    orb: super::Orb,
    player: super::Player,
    #[sprite_sheet_bundle]
    sprite_bundle: SpriteSheetBundle,
}

#[derive(Bundle, LdtkEntity)]
struct EnemyBundle {
    orb: super::Orb,
    enemy: super::Enemy,
    #[sprite_sheet_bundle]
    sprite_bundle: SpriteSheetBundle,
}

#[derive(Default, Component)]
struct LoadingScreenElement;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(LdtkWorldBundle {
        ldtk_handle: asset_server.load("levels.ldtk"),
        ..default()
    });

    commands.insert_resource(LevelSelection::Index(0));

    commands
        .spawn(TextBundle {
            text: Text::from_section(
                "Loading...",
                TextStyle {
                    color: Color::WHITE,
                    font_size: 72.0,
                    ..default()
                },
            ),
            ..default()
        })
        .insert(LoadingScreenElement);
}

fn detect_loaded(mut events: EventReader<LevelEvent>, mut next_state: ResMut<NextState<AppState>>) {
    for level_event in events.iter() {
        match level_event {
            LevelEvent::Transformed(_iid) => next_state.set(AppState::Playing),
            _ => (),
        }
    }
}

fn enable_tiles(
    enable: bool,
) -> impl Fn(
    Query<&mut Visibility, With<LevelSet>>,
    Query<&mut Visibility, (With<LoadingScreenElement>, Without<LevelSet>)>,
) -> () {
    move |mut levels, mut elements| {
        for mut level in levels.iter_mut() {
            *level.as_mut() = if enable {
                Visibility::Visible
            } else {
                Visibility::Hidden
            }
        }

        for mut element in elements.iter_mut() {
            *element.as_mut() = if enable {
                Visibility::Hidden
            } else {
                Visibility::Visible
            }
        }
    }
}

fn init_cells(
    mut commands: Commands,
    mut cells: Query<(Entity, &GridCoords, &IntGridCell, &mut Transform), Added<IntGridCell>>,
    tiles: Query<(&GridCoords, &TileMetadata)>,
) -> anyhow::Result<()> {
    let mut metadata_by_coords = HashMap::new();

    for (coords, metadata) in tiles.iter() {
        metadata_by_coords.insert(*coords, &metadata.data);
    }

    for (entity, coords, cell, mut transform) in cells.iter_mut() {
        let mut batch = commands.entity(entity);
        match cell.value {
            WALL_TILE => {
                batch
                    .insert(Collider::cuboid(128.0, 128.0))
                    .insert(Restitution::coefficient(1.0));
            }
            PIT_TILE => {
                batch.insert(Sensor).insert(ActiveEvents::COLLISION_EVENTS);

                if let Some(metadata) = metadata_by_coords.get(coords) {
                    let data: CustomData =
                        serde_json::from_str(metadata).context("deserialise CustomData")?;

                    let width = 256.0 - data.inset_left() - data.inset_right();
                    let height = 256.0 - data.inset_top() - data.inset_bottom();
                    let offset = Vec3::new(
                        data.inset_left() - data.inset_right(),
                        data.inset_bottom() - data.inset_top(),
                        0.0,
                    );

                    transform.translation += offset / 2.0;
                    batch.insert(Collider::cuboid(width / 2.0, height / 2.0));
                } else {
                    batch.insert(Collider::cuboid(128.0, 128.0));
                }
            }
            _ => (),
        }
    }
    Ok(())
}

fn init_entity(mut commands: Commands, mut query: Query<Entity, Added<super::Orb>>) {
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

pub struct LoaderPlugin;

impl Plugin for LoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LdtkPlugin)
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (init_cells.pipe(super::handle), init_entity, detect_loaded)
                    .run_if(in_state(AppState::Loading)),
            )
            .add_systems(OnEnter(AppState::Loading), enable_tiles(false))
            .add_systems(OnEnter(AppState::Playing), enable_tiles(true))
            .register_ldtk_entity::<PlayerBundle>("player")
            .register_ldtk_entity::<EnemyBundle>("enemy");
    }
}
