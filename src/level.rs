use crate::{ai, collision, vfx, AppState, CacheEvent, OpaquePlugin, Orb, PlayerInput, Tile};
use anyhow::Context;
use bevy::{
    math::Vec3Swizzles,
    prelude::*,
    sprite::Anchor,
    text::{Text2dBounds, TextLayoutInfo},
    utils::HashMap,
};
use bevy_ecs_ldtk::prelude::*;
use bevy_rapier2d::prelude::*;
use serde::Deserialize;

const WALL_TILE: i32 = 1;
const PIT_TILE: i32 = 2;
const MAX_LEVEL: usize = 4;

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

/// Blueprint bundle with extracted data and sprite
#[derive(Bundle, LdtkEntity)]
struct LdtkEntityBundle {
    #[with(LdtkOrb::new)]
    ldtk: LdtkOrb,
    #[sprite_sheet_bundle]
    sprite_bundle: SpriteSheetBundle,
}

///  Contains data from LDTK entities for blueprinting
#[derive(Component)]
struct LdtkOrb {
    identifier: String,
    mass: f32,
    sfx_name: &'static str,
    vfx_color: Vec4,
}

impl LdtkOrb {
    fn new(instance: &EntityInstance) -> LdtkOrb {
        LdtkOrb {
            identifier: instance.identifier.clone(),
            mass: instance.get_float_field("mass").cloned().unwrap_or(1.0),
            sfx_name: match instance.identifier.as_str() {
                "player" => "player-fall.ogg",
                _ => "enemy-fall.ogg",
            },
            vfx_color: match instance.identifier.as_str() {
                "player" => Vec4::new(0.2, 0.2, 1.0, 1.0),
                _ => Vec4::new(1.0, 0.1, 0.1, 1.0),
            },
        }
    }
}

// special bundle for on-screen text
#[derive(Bundle, LdtkEntity)]
struct TipBundle {
    #[with(LdtkTxt::new)]
    ldtk: LdtkTxt,
}

#[derive(Component)]
struct LdtkTxt {
    data: String,
}

impl LdtkTxt {
    fn new(instance: &EntityInstance) -> LdtkTxt {
        LdtkTxt {
            data: instance
                .get_string_field("data")
                .expect("missing txt.data")
                .clone(),
        }
    }
}

/// Marks pc, who must remain alive
#[derive(Component)]
struct Player;

/// Marks npc, who can be defeated
#[derive(Component)]
struct Enemy;

/// Marks a UI element hidden except while in loading state
#[derive(Component)]
struct LoadingScreenElement;

/// Cache of all pit locations in the current level
#[derive(Resource)]
pub struct LevelPits(Vec<Vec2>);

impl LevelPits {
    pub fn nearest_pit(&self, world_loc: &Vec2) -> Vec2 {
        let mut nearest = Vec2::MAX;
        let mut nearest_distance = f32::MAX;
        for &pit in self.0.iter() {
            let pit_distance = world_loc.distance(pit);
            if world_loc.distance(pit) < nearest_distance {
                nearest = pit - *world_loc;
                nearest_distance = pit_distance;
            }
        }
        nearest
    }
}

impl Default for LevelPits {
    fn default() -> Self {
        Self(default())
    }
}

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

fn cache_pit_locs(
    mut cache: ResMut<LevelPits>,
    mut input: EventReader<CacheEvent>,
    tiles: Query<(&Tile, &Transform)>,
) {
    if input
        .iter()
        .map(|event| matches!(event, CacheEvent::InvalidatePitCoords))
        .fold(false, |acc, x| acc || x)
    {
        cache.0.clear();
        for (_, transform) in tiles.iter().filter(|(tile, _)| matches!(tile, Tile::Pit)) {
            cache.0.push(transform.translation.xy());
        }
    }
}

fn detect_loaded(
    mut next_state: ResMut<NextState<AppState>>,
    mut level_events: EventReader<LevelEvent>,
    mut cache_events: EventWriter<CacheEvent>,
) {
    for level_event in level_events.iter() {
        match level_event {
            LevelEvent::Spawned(_) => {
                cache_events.send(CacheEvent::InvalidateColliderHierarchy);
                cache_events.send(CacheEvent::InvalidatePitCoords);
            }
            LevelEvent::Transformed(iid) => {
                info!("Loaded level {iid}");
                next_state.set(AppState::Playing);
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
                batch
                    .insert(Tile::Wall)
                    .with_children(collision::spawn_wall);
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

fn init_orb(
    mut commands: Commands,
    mut effects: ResMut<Assets<vfx::EffectAsset>>,
    mut query: Query<(Entity, &LdtkOrb), Added<LdtkOrb>>,
) {
    for (id, ldtk) in query.iter_mut() {
        let mut batch = commands.entity(id);

        // add physics
        batch
            .insert(RigidBody::Dynamic)
            .insert(Velocity::default())
            .insert(ExternalImpulse::default())
            .with_children(|children| collision::spawn_orb(children, ldtk.mass));

        // add movement and fall fx
        let effect_handle = vfx::allocate_thrust_sparks(&mut effects, ldtk.vfx_color);
        batch.insert(Orb {
            vfx: effect_handle,
            sfx: ldtk.sfx_name.into(),
        });

        // add gameplay
        match ldtk.identifier.as_str() {
            "player" => {
                batch.insert(Player).insert(PlayerInput);
            }
            "d_resignation" => {
                batch.insert(Enemy);
            }
            "d_intransigence" => {
                batch.insert(Enemy);
                ai::spawn_intransigence(&mut batch);
            }
            "d_cowardice" => {
                batch.insert(Enemy);
                ai::spawn_cowardice(&mut batch);
            }
            "d_malice" => {
                batch.insert(Enemy);
                ai::spawn_malice(&mut batch);
            }
            _ => {
                warn!("unknown LDTK entity '{}'", ldtk.identifier);
            }
        };
    }
}

fn init_txt(
    mut commands: Commands,
    mut query: Query<(Entity, &LdtkTxt, &mut Transform), Added<LdtkTxt>>,
) {
    for (id, ldtk, mut transform) in query.iter_mut() {
        let size = transform.scale.xy() * 256.0;
        transform.scale = Vec3::ONE;

        commands
            .entity(id)
            .insert(Text::from_section(
                &ldtk.data,
                TextStyle {
                    color: Color::WHITE,
                    font_size: 128.0,
                    ..default()
                },
            ))
            .insert(Text2dBounds { size })
            .insert(TextLayoutInfo::default())
            .insert(Anchor::Center);
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
            let next_level = (i + 1) % MAX_LEVEL;
            commands.insert_resource(LevelSelection::Index(next_level));
            next_state.set(AppState::Loading);
        }
    }
}

pub fn plugin(level_select: usize) -> impl Plugin {
    OpaquePlugin(move |app| {
        app.add_plugins(LdtkPlugin)
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    (
                        init_cells.pipe(super::handle),
                        init_orb,
                        init_txt,
                        detect_loaded,
                    )
                        .run_if(in_state(AppState::Loading)),
                    (respawn_after_death, advance_after_victory)
                        .run_if(in_state(AppState::Playing)),
                ),
            )
            .add_systems(PostUpdate, cache_pit_locs)
            .add_systems(OnEnter(AppState::Loading), enable_tiles(false))
            .add_systems(OnEnter(AppState::Playing), enable_tiles(true))
            .insert_resource(LevelSelection::Index(level_select))
            .init_resource::<LevelPits>()
            .register_default_ldtk_entity::<LdtkEntityBundle>()
            .register_ldtk_entity::<TipBundle>("txt");
    })
}
