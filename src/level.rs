use crate::{collision, Actor, AppState, CacheEvent, EnemyControl, PlayerControl, Tile};
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
    #[with(player_actor)]
    actor: Actor,
    player: Player,
    control: PlayerControl,
    #[sprite_sheet_bundle]
    sprite_bundle: SpriteSheetBundle,
}

#[derive(Bundle, LdtkEntity)]
struct DResBundle {
    enemy: Enemy,
    #[with(enemy_actor)]
    actor: Actor,
    #[sprite_sheet_bundle]
    sprite_bundle: SpriteSheetBundle,
}

#[derive(Bundle, LdtkEntity)]
struct DCowBundle {
    enemy: Enemy,
    #[with(dcow_control)]
    control: EnemyControl,
    #[with(enemy_actor)]
    actor: Actor,
    #[sprite_sheet_bundle]
    sprite_bundle: SpriteSheetBundle,
}

#[derive(Bundle, LdtkEntity)]
struct DMalBundle {
    enemy: Enemy,
    #[with(dmal_control)]
    control: EnemyControl,
    #[with(enemy_actor)]
    actor: Actor,
    #[sprite_sheet_bundle]
    sprite_bundle: SpriteSheetBundle,
}

fn player_actor(_: &EntityInstance) -> Actor {
    Actor {
        sfx: "player-fall.ogg".into(),
    }
}

fn enemy_actor(_: &EntityInstance) -> Actor {
    Actor {
        sfx: "player-fall.ogg".into(),
    }
}

fn dcow_control(_: &EntityInstance) -> EnemyControl {
    EnemyControl::Cowardice
}

fn dmal_control(_: &EntityInstance) -> EnemyControl {
    EnemyControl::Malice
}

/// Marks pc, who must remain alive
#[derive(Default, Component)]
struct Player;

/// Marks npc, who can be defeated
#[derive(Default, Component)]
struct Enemy;

#[derive(Default, Component)]
struct LoadingScreenElement;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(LdtkWorldBundle {
        ldtk_handle: asset_server.load("levels.ldtk"),
        ..default()
    });

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
    mut next_state: ResMut<NextState<AppState>>,
    mut level_events: EventReader<LevelEvent>,
    mut cache_events: EventWriter<CacheEvent>,
) {
    for level_event in level_events.iter() {
        match level_event {
            LevelEvent::Transformed(_iid) => {
                info!("Loaded level {_iid}");
                next_state.set(AppState::Playing);
                cache_events.send(CacheEvent::InvalidateColliderHierarchy)
            }
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
                        .insert(CollisionGroups::new(
                            collision::GROUP_WALL,
                            collision::FILTER_ALL,
                        ))
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

                    let entry_box = collision::Rect {
                        origin: offset,
                        size: Vec2::new(width, height),
                    };

                    let mut wall_boxes = Vec::new();

                    if data.inset_top() != 0.0 {
                        wall_boxes.push(collision::Rect {
                            origin: Vec2::new(0.0, 256.0 - data.inset_top()),
                            size: Vec2::new(256.0, data.inset_top()),
                        });
                    }

                    if data.inset_right() != 0.0 {
                        wall_boxes.push(collision::Rect {
                            origin: Vec2::new(256.0 - data.inset_right(), 0.0),
                            size: Vec2::new(data.inset_right(), 256.0),
                        });
                    }

                    if data.inset_bottom() != 0.0 {
                        wall_boxes.push(collision::Rect {
                            origin: Vec2::new(0.0, -128.0 - data.inset_bottom()),
                            size: Vec2::new(256.0, data.inset_bottom()),
                        });
                    }

                    if data.inset_left() != 0.0 {
                        wall_boxes.push(collision::Rect {
                            origin: Vec2::new(-128.0 - data.inset_left(), 0.0),
                            size: Vec2::new(data.inset_left(), 256.0),
                        });
                    }

                    (entry_box, wall_boxes)
                } else {
                    (
                        collision::Rect {
                            origin: Vec2::ZERO,
                            size: Vec2::new(256.0, 256.0),
                        },
                        Vec::<collision::Rect>::new(),
                    )
                };

                batch.insert(Tile::Pit).with_children(|children| {
                    collision::spawn_pit(children, &entry);
                    for wall in &walls {
                        collision::spawn_pit_wall(children, &wall);
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
                    .insert(CollisionGroups::new(
                        collision::GROUP_ACTOR,
                        collision::FILTER_MAIN,
                    ))
                    .insert(ColliderMassProperties::Mass(1.0))
                    .insert(Restitution::coefficient(1.0))
                    .insert(ActiveEvents::COLLISION_EVENTS);

                children
                    .spawn(Collider::ball(0.0))
                    .insert(CollisionGroups::new(
                        collision::GROUP_ONLY_ALL,
                        collision::FILTER_PITS,
                    ))
                    .insert(ColliderMassProperties::Mass(1.0))
                    .insert(Restitution::coefficient(1.0))
                    .insert(ActiveEvents::COLLISION_EVENTS);
            });
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

struct LevelPlugin {
    level_select: usize,
}

pub fn plugin(level_select: usize) -> impl Plugin {
    LevelPlugin { level_select }
}

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LdtkPlugin)
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    (init_cells.pipe(super::handle), init_entity, detect_loaded)
                        .run_if(in_state(AppState::Loading)),
                    (respawn_after_death, advance_after_victory)
                        .run_if(in_state(AppState::Playing)),
                ),
            )
            .add_systems(OnEnter(AppState::Loading), enable_tiles(false))
            .add_systems(OnEnter(AppState::Playing), enable_tiles(true))
            .insert_resource(LevelSelection::Index(self.level_select))
            .register_ldtk_entity::<PlayerBundle>("player")
            .register_ldtk_entity::<DResBundle>("d_resignation")
            .register_ldtk_entity::<DCowBundle>("d_cowardice")
            .register_ldtk_entity::<DMalBundle>("d_malice");
    }
}
