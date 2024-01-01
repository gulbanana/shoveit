use bevy::prelude::*;
use bevy_hanabi::prelude::*;
pub use bevy_hanabi::EffectAsset;
use std::time::Duration;

use crate::{AppState, OpaquePlugin};

const SPARK_DURATION: f32 = 2.0;
const SPARK_SPEED: f32 = 15.0;
const SPARK_COUNT: CpuValue<f32> = CpuValue::Uniform((4.0, 16.0));
const SPARK_SIZE: CpuValue<Vec2> = CpuValue::Uniform((Vec2::new(2.0, 2.0), Vec2::new(8.0, 8.0)));

#[derive(Component)]
struct Lifespan(Duration);

fn live_fast_die_young(
    elapsed: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Lifespan)>,
) {
    for (entity, mut lifespan) in query.iter_mut() {
        if lifespan.0 > Duration::ZERO {
            let diff = elapsed.delta().clamp(Duration::ZERO, lifespan.0);
            lifespan.0 -= diff;
        } else {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn plugin() -> impl Plugin {
    OpaquePlugin(|app| {
        app.add_plugins(HanabiPlugin).add_systems(
            Update,
            live_fast_die_young.run_if(in_state(AppState::Playing)),
        );
    })
}

pub fn allocate_thrust_sparks(
    effects: &mut ResMut<Assets<EffectAsset>>,
    key_color: Vec4,
) -> Handle<EffectAsset> {
    let mut gradient = Gradient::new();
    gradient.add_key(0.0, key_color);
    gradient.add_key(1.0, Vec4::splat(0.0));
    let render_color = ColorOverLifetimeModifier { gradient };

    //exprs are stored in a module, effect as a whole is stored in an asset
    let mut module = Module::default();

    let render_size = SetSizeModifier {
        size: SPARK_SIZE,
        screen_space_size: false,
    };

    let init_position = SetPositionCircleModifier {
        center: module.lit(Vec3::ZERO),
        radius: module.lit(100.0), // standard orb size
        axis: module.lit(Vec3::Z),
        dimension: ShapeDimension::Volume,
    };

    let init_velocity = SetAttributeModifier::new(Attribute::VELOCITY, module.prop("vector"));

    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, module.lit(SPARK_DURATION));

    let mut effect = EffectAsset::new(32768, Spawner::once(SPARK_COUNT, true), module)
        .with_name("ThrustSparks")
        .with_property("vector", Vec3::ZERO.into())
        .init(init_position)
        .init(init_lifetime)
        .init(init_velocity)
        .render(render_size)
        .render(render_color);

    effect.z_layer_2d = 4.0; // beneath entity layer

    return effects.add(effect);
}

pub fn instantiate_thrust_sparks(
    children: &mut ChildBuilder,
    effect_handle: Handle<EffectAsset>,
    impulse: Vec2,
) {
    let inverse_impulse = Vec3::new(
        0.0 - impulse.x * SPARK_SPEED,
        0.0 - impulse.y * SPARK_SPEED,
        0.0,
    );
    children
        .spawn(ParticleEffectBundle {
            effect: ParticleEffect::new(effect_handle)
                .with_properties::<()>(vec![("vector".to_owned(), inverse_impulse.into())]),
            ..default()
        })
        .insert(Lifespan(Duration::from_secs_f32(SPARK_DURATION)));
}
