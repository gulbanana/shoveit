use crate::{COLLISION_MAIN, COLLISION_PIT_ENTRY, COLLISION_PIT_WALLS};

use super::{AppState, Tile};
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
    orb: super::Actor,
    player: super::Player,
    #[sprite_sheet_bundle]
    sprite_bundle: SpriteSheetBundle,
}

#[derive(Bundle, LdtkEntity)]
struct EnemyBundle {
    orb: super::Actor,
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

fn detect_loaded(
    mut events: EventReader<LevelEvent>,
    mut next_state: ResMut<NextState<AppState>>,
    level: Res<LevelSelection>,
) {
    if let LevelSelection::Index(i) = level.into_inner() {
        info!("Loaded level {i}");
    }

    for level_event in events.iter() {
        match level_event {
            LevelEvent::Spawned(_iid) => next_state.set(AppState::Playing),
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

struct ColliderRect {
    origin: Vec2,
    size: Vec2,
}

fn init_cells(
    mut commands: Commands,
    mut cells: Query<(Entity, &GridCoords, &IntGridCell), Added<IntGridCell>>,
    tiles: Query<(&GridCoords, &TileMetadata)>,
) -> anyhow::Result<()> {
    let mut metadata_by_coords = HashMap::new();

    for (coords, metadata) in tiles.iter() {
        metadata_by_coords.insert(*coords, &metadata.data);
    }

    for (entity, coords, cell) in cells.iter_mut() {
        let mut batch = commands.entity(entity);
        batch.insert(RigidBody::Fixed);

        match cell.value {
            WALL_TILE => {
                batch.insert(Tile::Wall).with_children(|children| {
                    children
                        .spawn(Collider::cuboid(128.0, 128.0))
                        .insert(CollisionGroups::new(COLLISION_MAIN, Group::ALL))
                        .insert(Restitution::coefficient(1.0));
                });
            }
            PIT_TILE => {
                let (entry, walls) = if let Some(metadata) = metadata_by_coords.get(coords) {
                    let data: CustomData =
                        serde_json::from_str(metadata).context("deserialise CustomData")?;

                    let width = 256.0 - data.inset_left() - data.inset_right();
                    let height = 256.0 - data.inset_top() - data.inset_bottom();
                    let offset = Vec2::new(
                        data.inset_left() - data.inset_right(),
                        data.inset_bottom() - data.inset_top(),
                    );

                    let entry_box = ColliderRect {
                        origin: offset,
                        size: Vec2::new(width, height),
                    };

                    let mut wall_boxes = Vec::new();

                    if data.inset_top() != 0.0 {
                        wall_boxes.push(ColliderRect {
                            origin: Vec2::new(0.0, 256.0 - data.inset_top()),
                            size: Vec2::new(256.0, data.inset_top()),
                        });
                    }

                    if data.inset_right() != 0.0 {
                        wall_boxes.push(ColliderRect {
                            origin: Vec2::new(256.0 - data.inset_right(), 0.0),
                            size: Vec2::new(data.inset_right(), 256.0),
                        });
                    }

                    if data.inset_bottom() != 0.0 {
                        wall_boxes.push(ColliderRect {
                            origin: Vec2::new(0.0, -128.0 - data.inset_bottom()),
                            size: Vec2::new(256.0, data.inset_bottom()),
                        });
                    }

                    if data.inset_left() != 0.0 {
                        wall_boxes.push(ColliderRect {
                            origin: Vec2::new(-128.0 - data.inset_left(), 0.0),
                            size: Vec2::new(data.inset_left(), 256.0),
                        });
                    }

                    (entry_box, wall_boxes)
                } else {
                    (
                        ColliderRect {
                            origin: Vec2::ZERO,
                            size: Vec2::new(256.0, 256.0),
                        },
                        Vec::<ColliderRect>::new(),
                    )
                };

                batch.insert(Tile::Pit).with_children(|children| {
                    children
                        .spawn(SpatialBundle::from_transform(Transform::from_xyz(
                            entry.origin.x / 2.0,
                            entry.origin.y / 2.0,
                            0.0,
                        )))
                        .insert(Collider::cuboid(entry.size.x / 2.0, entry.size.y / 2.0))
                        .insert(CollisionGroups::new(COLLISION_PIT_ENTRY, Group::ALL))
                        .insert(Sensor);

                    for wall in walls {
                        children
                            .spawn(SpatialBundle::from_transform(Transform::from_xyz(
                                wall.origin.x / 2.0,
                                wall.origin.y / 2.0,
                                0.0,
                            )))
                            .insert(Collider::cuboid(wall.size.x / 2.0, wall.size.y / 2.0))
                            .insert(CollisionGroups::new(COLLISION_PIT_WALLS, Group::ALL))
                            .insert(Restitution::coefficient(1.0));
                    }
                });
            }
            _ => (),
        }
    }
    Ok(())
}

fn init_entity(mut commands: Commands, mut query: Query<Entity, Added<super::Actor>>) {
    for id in query.iter_mut() {
        commands
            .entity(id)
            .insert(RigidBody::Dynamic)
            .insert(Velocity::default())
            .insert(ExternalImpulse::default())
            .with_children(|children| {
                children
                    .spawn(Collider::ball(100.0))
                    .insert(CollisionGroups::new(COLLISION_MAIN, COLLISION_MAIN))
                    .insert(ColliderMassProperties::Mass(1.0))
                    .insert(Restitution::coefficient(1.0))
                    .insert(ActiveEvents::COLLISION_EVENTS);

                children
                    .spawn(Collider::ball(0.0))
                    .insert(CollisionGroups::new(
                        COLLISION_PIT_ENTRY,
                        COLLISION_PIT_ENTRY,
                    ))
                    .insert(ColliderMassProperties::Mass(1.0))
                    .insert(Restitution::coefficient(1.0))
                    .insert(ActiveEvents::COLLISION_EVENTS);
            });
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
