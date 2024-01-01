use std::time::Duration;

use bevy::{prelude::*, render::camera::ScalingMode};
use bevy_rapier2d::prelude::*;
use bevy_tweening::{lens::TransformScaleLens, *};

mod ai;
mod collision;
mod level;
mod movement;
mod vfx;

// pixels per second
const MAX_V: f32 = 3000.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States)]
enum AppState {
    #[default]
    Loading,
    Playing,
}

/// Player button presses
#[derive(Event)]
enum InputEvent {
    Decelerate,
    Accelerate(Vec2),
}

/// Interactions detected by physics
#[derive(Event)]
enum InteractionEvent {
    OrbHitOrb,
    OrbHitWall,
    OrbHitPit(Entity),
}

#[derive(Event)]
enum CacheEvent {
    InvalidateColliderHierarchy,
}

/// Has interactions on contact
#[derive(Component)]
enum Tile {
    Wall,
    Pit,
}

/// Moves around the level, interacting with other orbs and with tiles
#[derive(Component)]
struct Orb {
    sfx: String,
    vfx: Handle<vfx::EffectAsset>,
}

#[derive(Component, Default)]
struct PlayerControl;

#[derive(Component)]
enum EnemyControl {
    Cowardice,
    Malice,
}

fn setup(mut commands: Commands) {
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
}

fn keyboard_input(input: Res<Input<KeyCode>>, mut events: EventWriter<InputEvent>) {
    // braking takes priority
    if input.pressed(KeyCode::Space) {
        events.send(InputEvent::Decelerate);
        return;
    }

    // if not braking, we may thrust
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

    if thrust != Vec2::ZERO {
        thrust = thrust.normalize();
        events.send(InputEvent::Accelerate(thrust));
    }
}

fn move_player(
    time: Res<Time>,
    mut events: EventReader<InputEvent>,
    mut query: Query<
        (&mut Transform, &mut Velocity, &mut ExternalImpulse),
        (With<PlayerControl>, With<Orb>),
    >,
) {
    for event in events.iter() {
        match *event {
            InputEvent::Decelerate => {
                for (_, velocity, mut impulse) in query.iter_mut() {
                    movement::decelerate_orb(&time, velocity.as_ref(), impulse.as_mut())
                }
            }
            InputEvent::Accelerate(thrust) => {
                for (mut transform, mut velocity, mut impulse) in query.iter_mut() {
                    movement::accelerate_orb(
                        &time,
                        thrust,
                        transform.as_mut(),
                        velocity.as_mut(),
                        impulse.as_mut(),
                    );
                }
            }
        }
    }
}

fn cap_velocity(mut query: Query<&mut Velocity, With<Orb>>) {
    for mut velocity in query.iter_mut() {
        velocity.linvel = velocity.linvel.clamp_length_max(MAX_V);
    }
}

fn trigger_vfx(mut commands: Commands, mut query: Query<(Entity, &Orb, &ExternalImpulse)>) {
    for (entity, orb, impulse) in query.iter_mut() {
        if impulse.impulse != Vec2::ZERO {
            commands.entity(entity).with_children(|children| {
                vfx::instantiate_thrust_sparks(children, orb.vfx.clone(), impulse.impulse);
            });
        }
    }
}

fn trigger_interaction(
    assets: Res<AssetServer>,
    mut commands: Commands,
    mut events: EventReader<InteractionEvent>,
    orbs: Query<&Orb>,
) {
    for event in events.iter() {
        match event {
            InteractionEvent::OrbHitWall => {
                commands.spawn(AudioBundle {
                    source: assets.load("pobble.ogg"),
                    ..default()
                });
            }
            InteractionEvent::OrbHitOrb => {
                commands.spawn(AudioBundle {
                    source: assets.load("pobblebonk.ogg"),
                    ..default()
                });
            }
            InteractionEvent::OrbHitPit(entity) => {
                if let Ok(orb) = orbs.get(*entity) {
                    commands.spawn(AudioBundle {
                        source: assets.load(&orb.sfx),
                        ..default()
                    });
                }

                // shrink into oblivion
                let tween = Tween::new(
                    EaseFunction::QuadraticIn,
                    Duration::from_millis(1500),
                    TransformScaleLens {
                        start: Vec3::ONE,
                        end: Vec3::ZERO,
                    },
                )
                .with_completed_event(0);

                commands
                    .entity(*entity)
                    .remove::<Orb>()
                    .insert(Animator::new(tween))
                    .despawn_descendants()
                    .with_children(|children| {
                        collision::spawn_falling_orb(children);
                    });
            }
        }
    }
}

fn die_after_fall(
    mut commands: Commands,
    mut tween_events: EventReader<TweenCompleted>,
    mut cache_events: EventWriter<CacheEvent>,
) {
    for fallen in tween_events.iter() {
        commands.entity(fallen.entity).despawn_recursive();
        cache_events.send(CacheEvent::InvalidateColliderHierarchy);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut level_select = 1;
    if let Some(arg1) = args.get(1) {
        if let Ok(index) = arg1.parse() {
            level_select = index;
        }
    }

    App::new()
        .add_plugins((
            DefaultPlugins
                // .set(bevy::log::LogPlugin {
                //     filter: "wgpu=error,naga=warn,big_brain=debug".to_string(),
                //     ..default()
                // })
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Shove it!".into(),
                        ..default()
                    }),
                    ..default()
                }),
            TweeningPlugin,
            ai::plugin(),
            level::plugin(level_select),
            collision::plugin(),
            vfx::plugin(),
        ))
        .add_state::<AppState>()
        .add_event::<InputEvent>()
        .add_event::<InteractionEvent>()
        .add_event::<CacheEvent>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                keyboard_input.before(move_player),
                move_player.before(cap_velocity),
                cap_velocity,
                trigger_vfx.after(move_player),
                trigger_interaction,
                die_after_fall,
            )
                .run_if(in_state(AppState::Playing)),
        )
        .run();
}

struct OpaquePlugin<T>(T)
where
    T: Fn(&mut App);

impl<T> Plugin for OpaquePlugin<T>
where
    T: Fn(&mut App) + Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        self.0(app);
    }
}

pub fn handle(result: In<anyhow::Result<()>>) {
    if let In(Result::Err(cause)) = result {
        error!("{}", cause);
    }
}
