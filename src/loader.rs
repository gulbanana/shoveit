use super::AppState;
use bevy::prelude::*;
use bevy_ecs_ldtk::prelude::*;
use bevy_rapier2d::prelude::*;

const WALL_TILE: i32 = 1;
const PIT_TILE: i32 = 2;

#[derive(Default, Component)]
pub struct LoadingScreenElement;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
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

pub fn detect_loaded(
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

pub fn enable_tiles(
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
            };
        }

        for mut element in elements.iter_mut() {
            *element.as_mut() = if enable {
                Visibility::Hidden
            } else {
                Visibility::Visible
            };
        }
    }
}

pub fn init_cells(
    mut commands: Commands,
    mut query: Query<(Entity, &IntGridCell), Added<IntGridCell>>,
) {
    for (id, cell) in query.iter_mut() {
        match cell.value {
            WALL_TILE => {
                commands
                    .entity(id)
                    .insert(Collider::cuboid(128.0, 128.0))
                    .insert(Restitution::coefficient(1.0));
            }
            PIT_TILE => {
                commands
                    .entity(id)
                    .insert(Collider::cuboid(64.0, 64.0))
                    .insert(Sensor)
                    .insert(ActiveEvents::COLLISION_EVENTS);
            }
            _ => (),
        }
    }
}

pub fn init_player(mut commands: Commands, mut query: Query<Entity, Added<super::Player>>) {
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
