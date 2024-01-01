use crate::{EnemyControl, OpaquePlugin, Orb, PlayerControl};
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
    player: Query<&Transform, With<PlayerControl>>,
    mut orbs: Query<
        (&mut Transform, &mut Velocity, &mut ExternalImpulse),
        (With<Orb>, Without<PlayerControl>),
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
struct Stop;

fn stop_action(
    time: Res<Time>,
    mut orbs: Query<
        (&mut Transform, &mut Velocity, &mut ExternalImpulse),
        (With<Orb>, Without<PlayerControl>),
    >,
    mut actions: Query<(&Actor, &mut ActionState), With<Stop>>,
) {
    for (Actor(actor), mut state) in actions.iter_mut() {
        if let Ok((_, mut velocity, mut impulse)) = orbs.get_mut(*actor) {
            match *state {
                ActionState::Requested => {
                    crate::movement::decelerate_orb(&time, velocity.as_mut(), impulse.as_mut());
                    *state = ActionState::Executing;
                }
                ActionState::Executing => {
                    if velocity.linvel == Vec2::ZERO {
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

#[derive(Clone, Component, Debug, ScorerBuilder)]
struct Charge;

fn flee_scorer(
    player: Query<&Transform, With<PlayerControl>>,
    orbs: Query<(&Transform, &EnemyControl)>,
    mut scorers: Query<(&Actor, &mut Score), With<Flee>>,
) {
    if let Ok(Transform {
        translation: player_loc,
        ..
    }) = player.get_single()
    {
        for (Actor(actor), mut score) in &mut scorers {
            if let Ok((
                Transform {
                    translation: enemy_loc,
                    ..
                },
                EnemyControl::Cowardice,
            )) = orbs.get(*actor)
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

fn charge_scorer(
    player: Query<&Transform, With<PlayerControl>>,
    orbs: Query<(&Transform, &EnemyControl)>,
    mut scorers: Query<(&Actor, &mut Score), With<Charge>>,
) {
    if let Ok(Transform {
        translation: player_loc,
        ..
    }) = player.get_single()
    {
        for (Actor(actor), mut score) in &mut scorers {
            if let Ok((
                Transform {
                    translation: enemy_loc,
                    ..
                },
                EnemyControl::Malice,
            )) = orbs.get(*actor)
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

#[derive(Clone, Component, Debug, ScorerBuilder)]
struct Bored;

fn bored_scorer(
    orbs: Query<(&Transform, &EnemyControl)>,
    mut scorers: Query<(&Actor, &mut Score), With<Bored>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        if let Ok(_) = orbs.get(*actor) {
            score.set(0.01);
        }
    }
}

pub fn plugin() -> impl Plugin {
    OpaquePlugin(|app| {
        app.add_plugins(BigBrainPlugin::new(PreUpdate))
            .add_systems(
                PreUpdate,
                (move_action, stop_action).in_set(BigBrainSet::Actions),
            )
            .add_systems(
                PreUpdate,
                (flee_scorer, charge_scorer, bored_scorer).in_set(BigBrainSet::Scorers),
            );
    })
}

pub fn insert_thinker(entity: &mut EntityCommands) {
    entity.insert(
        Thinker::build()
            .label("EnemyControl")
            .picker(Highest)
            .when(Flee, Move::Away)
            .when(Charge, Move::Toward)
            .when(Bored, Stop),
    );
}
