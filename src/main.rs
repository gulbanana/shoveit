use bevy::prelude::*;
use bevy::{math::Vec3Swizzles, render::camera::ScalingMode};
use bevy_hanabi::prelude::*;
use bevy_rapier2d::prelude::*;
use bevy_tweening::{lens::TransformScaleLens, *};
use std::f32::consts::PI;
use std::time::Duration;

mod collision;
mod level;

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
    ActorHitActor,
    ActorHitWall,
    ActorEnterPit(Entity),
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

/// Moves around the level, interacting with other actors and with tiles
#[derive(Component)]
struct Actor {
    sfx: String,
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
        (With<PlayerControl>, With<Actor>),
    >,
) {
    for event in events.iter() {
        match *event {
            InputEvent::Decelerate => {
                for (_, velocity, mut impulse) in query.iter_mut() {
                    let antithrust = velocity.linvel.normalize();
                    impulse.impulse = (antithrust * -1500.0 * time.delta_seconds())
                        .clamp_length(0.0, velocity.linvel.length());
                }
            }
            InputEvent::Accelerate(thrust) => {
                for (mut transform, mut velocity, mut impulse) in query.iter_mut() {
                    let forward = (transform.rotation * Vec3::Y).xy();
                    let forward_dot_goal = forward.dot(thrust);

                    // if facing ⋅ thrust is significant, rotate towards thrust
                    if (forward_dot_goal - 1.0).abs() >= f32::EPSILON {
                        // cancel any tumbling
                        velocity.angvel = 0.0;

                        // +ve=anticlockwise, -ve=clockwise (right hand rule)
                        let right = (transform.rotation * Vec3::X).xy();
                        let right_dot_goal = right.dot(thrust);
                        let sign = -f32::copysign(1.0, right_dot_goal);

                        // avoid overshoot
                        let max_angle = forward_dot_goal.clamp(-1.0, 1.0).acos();
                        let rotation_angle =
                            (sign * 4.0 * PI * time.delta_seconds()).min(max_angle);

                        transform.rotate_z(rotation_angle);
                    }
                    // otherwise, apply thrust in the direction we are now facing
                    else {
                        impulse.impulse = thrust * 750.0 * time.delta_seconds();
                    }
                }
            }
        }
    }
}

fn cap_player_velocity(mut query: Query<&mut Velocity, With<PlayerControl>>) {
    for mut velocity in query.iter_mut() {
        velocity.linvel = velocity.linvel.clamp_length_max(3000.0);
    }
}

fn trigger_vfx(mut query: Query<(&Velocity, &mut EffectSpawner), With<Actor>>) {
    for (velocity, mut spawner) in query.iter_mut() {
        if velocity.linvel != Vec2::ZERO {
            spawner.reset();
        }
    }
}

fn trigger_interaction(
    assets: Res<AssetServer>,
    mut commands: Commands,
    mut events: EventReader<InteractionEvent>,
    actors: Query<&Actor>,
) {
    for event in events.iter() {
        match event {
            InteractionEvent::ActorHitWall => {
                commands.spawn(AudioBundle {
                    source: assets.load("pobble.ogg"),
                    ..default()
                });
            }
            InteractionEvent::ActorHitActor => {
                commands.spawn(AudioBundle {
                    source: assets.load("pobblebonk.ogg"),
                    ..default()
                });
            }
            InteractionEvent::ActorEnterPit(actor) => {
                if let Ok(actor) = actors.get(*actor) {
                    commands.spawn(AudioBundle {
                        source: assets.load(&actor.sfx),
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
                    .entity(*actor)
                    .remove::<Actor>()
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
    let mut level_select = 0;
    if let Some(arg1) = args.get(1) {
        if let Ok(index) = arg1.parse() {
            level_select = index;
        }
    }

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
            TweeningPlugin,
            HanabiPlugin,
            level::plugin(level_select),
            collision::plugin(),
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
                move_player.before(cap_player_velocity),
                cap_player_velocity,
                trigger_vfx,
                trigger_interaction,
                die_after_fall,
            )
                .run_if(in_state(AppState::Playing)),
        )
        .run();
}

pub fn handle(result: In<anyhow::Result<()>>) {
    if let In(Result::Err(cause)) = result {
        error!("{}", cause);
    }
}

pub fn create_vfx(
    effects: &mut ResMut<Assets<EffectAsset>>,
    key_color: Vec4,
) -> Handle<EffectAsset> {
    let mut gradient = Gradient::new();
    gradient.add_key(0.0, key_color);
    gradient.add_key(1.0, Vec4::splat(0.0));
    let render_color = ColorOverLifetimeModifier { gradient };

    let mut module = Module::default();

    let render_size = SetSizeModifier {
        size: CpuValue::Uniform((Vec2::new(5.0, 5.0), Vec2::new(10.0, 10.0))),
        screen_space_size: false,
    };

    // pos = c + r * dir
    let init_pos = SetPositionCircleModifier {
        center: module.lit(Vec3::ZERO),
        radius: module.lit(100.0),
        axis: module.lit(Vec3::Z),
        dimension: ShapeDimension::Surface,
    };

    // radial velocity (away from center)
    let init_vel = SetVelocityCircleModifier {
        center: module.lit(Vec3::ZERO),
        axis: module.lit(Vec3::Z),
        speed: module.lit(1.0),
    };

    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, module.lit(3.0));

    let mut effect = EffectAsset::new(32768, Spawner::once(1.0.into(), false), module)
        .with_name("OrbRing")
        .init(init_pos)
        .init(init_vel)
        .init(init_lifetime)
        .render(render_size)
        .render(render_color);

    effect.z_layer_2d = 4.0; // XXX investigate why this works and 500.0 does not

    // Insert into the asset system
    return effects.add(effect);
}
