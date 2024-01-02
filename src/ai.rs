use crate::{OpaquePlugin, Orb, PlayerInput};
use bevy::{ecs::system::EntityCommands, math::Vec3Swizzles, prelude::*};
use bevy_rapier2d::prelude::*;
use big_brain::prelude::*;

#[derive(Clone, Component, Debug, ActionBuilder)]
enum Move {
    Toward,
    Away,
}

fn move_action(
    time: Res<Time>,
    player: Query<&Transform, With<PlayerInput>>,
    mut orbs: Query<
        (&mut Transform, &mut Velocity, &mut ExternalImpulse),
        (With<Orb>, Without<PlayerInput>),
    >,
    mut actions: Query<(&Actor, &mut ActionState, &Move)>,
) {
    if let Ok(Transform {
        translation: player_loc,
        ..
    }) = player.get_single()
    {
        for (Actor(actor), mut state, action) in actions.iter_mut() {
            if let Ok((mut transform, mut velocity, mut impulse)) = orbs.get_mut(*actor) {
                let vector_to_player = *player_loc - transform.translation;
                let thrust = match action {
                    Move::Toward => vector_to_player.normalize().xy(),
                    Move::Away => -vector_to_player.normalize().xy(),
                };

                let distance_to_player = vector_to_player.length() / 256.0;
                let reached_goal = match action {
                    Move::Toward => distance_to_player <= 3.0,
                    Move::Away => distance_to_player > 3.0,
                };

                match *state {
                    ActionState::Requested => {
                        crate::movement::accelerate_orb(
                            &time,
                            thrust,
                            transform.as_mut(),
                            velocity.as_mut(),
                            impulse.as_mut(),
                        );
                        *state = ActionState::Executing;
                    }
                    ActionState::Executing => {
                        if reached_goal {
                            *state = ActionState::Success;
                        } else {
                            crate::movement::accelerate_orb(
                                &time,
                                thrust,
                                transform.as_mut(),
                                velocity.as_mut(),
                                impulse.as_mut(),
                            );
                        }
                    }
                    ActionState::Cancelled => {
                        *state = ActionState::Failure;
                    }
                    _ => (),
                }
            }
        }
    }
}

#[derive(Clone, Component, Debug, ActionBuilder)]
struct Halt;

fn halt_action(
    time: Res<Time>,
    mut orbs: Query<
        (&mut Transform, &mut Velocity, &mut ExternalImpulse),
        (With<Orb>, Without<PlayerInput>),
    >,
    mut actions: Query<(&Actor, &mut ActionState), With<Halt>>,
) {
    for (Actor(actor), mut state) in actions.iter_mut() {
        if let Ok((_, mut velocity, mut impulse)) = orbs.get_mut(*actor) {
            match *state {
                ActionState::Requested => {
                    crate::movement::decelerate_orb(&time, velocity.as_mut(), impulse.as_mut());
                    *state = ActionState::Executing;
                }
                ActionState::Executing => {
                    if velocity.angvel == 0.0 && velocity.linvel == Vec2::ZERO {
                        *state = ActionState::Success;
                    } else {
                        crate::movement::decelerate_orb(&time, velocity.as_mut(), impulse.as_mut());
                    }
                }
                ActionState::Cancelled => {
                    *state = ActionState::Failure;
                }
                _ => (),
            }
        }
    }
}

#[derive(Clone, Component, Debug, ScorerBuilder)]
struct Flee;

fn flee_scorer(
    player: Query<&Transform, With<PlayerInput>>,
    enemies: Query<&Transform, Without<PlayerInput>>,
    mut scorers: Query<(&Actor, &mut Score), With<Flee>>,
) {
    if let Ok(Transform {
        translation: player_loc,
        ..
    }) = player.get_single()
    {
        for (Actor(actor), mut score) in &mut scorers {
            if let Ok(Transform {
                translation: enemy_loc,
                ..
            }) = enemies.get(*actor)
            {
                let distance_to_player = enemy_loc.distance(*player_loc) / 256.0;
                let distance_within_3 = (3.0 - distance_to_player).clamp(0.0, 3.0);

                if !distance_within_3.is_nan() {
                    score.set(distance_within_3 / 3.0);
                }
            }
        }
    }
}

#[derive(Clone, Component, Debug, ScorerBuilder)]
struct Charge;

fn charge_scorer(
    player: Query<&Transform, With<PlayerInput>>,
    enemies: Query<&Transform, Without<PlayerInput>>,
    mut scorers: Query<(&Actor, &mut Score), With<Charge>>,
) {
    if let Ok(Transform {
        translation: player_loc,
        ..
    }) = player.get_single()
    {
        for (Actor(actor), mut score) in &mut scorers {
            if let Ok(Transform {
                translation: enemy_loc,
                ..
            }) = enemies.get(*actor)
            {
                let distance_to_player = enemy_loc.distance(*player_loc) / 256.0;
                let distance_beyond_3 = (distance_to_player - 3.0).clamp(0.0, 3.0);

                if !distance_beyond_3.is_nan() {
                    score.set(distance_beyond_3 / 3.0);
                }
            }
        }
    }
}

// low-value desire to stop moving
#[derive(Clone, Component, Debug, ScorerBuilder)]
struct Moving;

fn moving_scorer(orbs: Query<&Velocity>, mut scorers: Query<(&Actor, &mut Score), With<Moving>>) {
    for (Actor(actor), mut score) in &mut scorers {
        if let Ok(velocity) = orbs.get(*actor) {
            if velocity.angvel != 0.0 || velocity.linvel != Vec2::ZERO {
                score.set(0.1);
            }
        }
    }
}

pub fn plugin() -> impl Plugin {
    OpaquePlugin(|app| {
        app.add_plugins(BigBrainPlugin::new(PreUpdate))
            .add_systems(
                PreUpdate,
                (move_action, halt_action).in_set(BigBrainSet::Actions),
            )
            .add_systems(
                PreUpdate,
                (flee_scorer, charge_scorer, moving_scorer).in_set(BigBrainSet::Scorers),
            );
    })
}

pub fn spawn_intransigence(entity: &mut EntityCommands) {
    entity.insert(
        Thinker::build()
            .label("EnemyControl")
            .picker(Highest)
            .when(Moving, Halt),
    );
}

pub fn spawn_cowardice(entity: &mut EntityCommands) {
    entity.insert(
        Thinker::build()
            .label("EnemyControl")
            .picker(Highest)
            .when(Flee, Move::Away)
            .when(Moving, Halt),
    );
}

pub fn spawn_malice(entity: &mut EntityCommands) {
    entity.insert(
        Thinker::build()
            .label("EnemyControl")
            .picker(Highest)
            .when(Charge, Move::Toward)
            .when(Moving, Halt),
    );
}
