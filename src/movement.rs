use bevy::{math::Vec3Swizzles, prelude::*};
use bevy_rapier2d::prelude::*;
use std::f32::consts::PI;

// pixels per second per second
const ACCEL_V: f32 = 750.0;
const DECEL_V: f32 = -1500.0;

pub fn accelerate_orb(
    time: &Res<Time>,
    thrust: Vec2, // desired vector, normalised
    transform: &mut Transform,
    velocity: &mut Velocity,
    impulse: &mut ExternalImpulse,
) {
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
        let rotation_angle = (sign * 4.0 * PI * time.delta_seconds()).min(max_angle);

        transform.rotate_z(rotation_angle);
    }
    // otherwise, apply thrust in the direction we are now facing
    else {
        impulse.impulse = thrust * ACCEL_V * time.delta_seconds();
    }
}

pub fn decelerate_orb(time: &Res<Time>, velocity: &mut Velocity, impulse: &mut ExternalImpulse) {
    velocity.angvel = 0.0; // cheap, but w/e

    let mut antithrust = velocity.linvel.normalize();
    antithrust = antithrust * DECEL_V * time.delta_seconds();
    antithrust = antithrust.clamp_length(0.0, velocity.linvel.length());

    if !antithrust.is_nan() {
        impulse.impulse = antithrust;
    }
}
