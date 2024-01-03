use crate::{level::LevelPits, OpaquePlugin, Orb, PlayerInput};
use bevy::{ecs::system::EntityCommands, math::Vec3Swizzles, prelude::*};
use bevy_rapier2d::prelude::*;
use big_brain::prelude::*;
use std::time::Duration;

const MIN_THRUST_PERIOD: Duration = Duration::from_millis(100);

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

#[derive(Clone, Component, Debug, ActionBuilder)]
struct RelativeMove {
    r#type: MoveType,
    thrust: Option<Vec2>,
    since: Duration,
}

impl From<MoveType> for RelativeMove {
    fn from(value: MoveType) -> Self {
        RelativeMove {
            r#type: value,
            thrust: None,
            since: Duration::ZERO,
        }
    }
}

#[derive(Clone, Debug)]
enum MoveType {
    AvoidPit,
    AvoidPlayer,
    ChasePlayer,
}

fn relative_move_action(
    time: Res<Time>,
    pits: Res<LevelPits>,
    player: Query<&Transform, With<PlayerInput>>,
    mut orbs: Query<
        (&mut Transform, &mut Velocity, &mut ExternalImpulse),
        (With<Orb>, Without<PlayerInput>),
    >,
    mut actions: Query<(&Actor, &mut ActionState, &mut RelativeMove)>,
) {
    for (Actor(actor), mut state, mut action) in actions.iter_mut() {
        if let Ok((mut transform, mut velocity, mut impulse)) = orbs.get_mut(*actor) {
            let (precondition_failed, reached_goal, mut thrust) = match action.r#type {
                MoveType::AvoidPit => {
                    let vector_to_pit = pits.nearest_pit(&transform.translation.xy());
                    let distance_to_pit = vector_to_pit.length() / 256.0;
                    (false, distance_to_pit >= 3.0, -vector_to_pit.normalize())
                }
                MoveType::AvoidPlayer => {
                    if let Ok(Transform {
                        translation: player_loc,
                        ..
                    }) = player.get_single()
                    {
                        let vector_to_player = *player_loc - transform.translation;
                        let distance_to_player = vector_to_player.length() / 256.0;
                        (
                            false,
                            distance_to_player >= 3.0,
                            -vector_to_player.normalize().xy(),
                        )
                    } else {
                        (true, false, Vec2::ZERO)
                    }
                }
                MoveType::ChasePlayer => {
                    if let Ok(Transform {
                        translation: player_loc,
                        ..
                    }) = player.get_single()
                    {
                        let vector_to_player = *player_loc - transform.translation;
                        let distance_to_player = vector_to_player.length() / 256.0;
                        (
                            false,
                            distance_to_player <= 3.0,
                            vector_to_player.normalize().xy(),
                        )
                    } else {
                        (true, false, Vec2::ZERO)
                    }
                }
            };

            debug!("RelativeMove spec: failed({precondition_failed}) completed({reached_goal}) thrust({thrust})");

            if precondition_failed {
                *state = ActionState::Failure;
            } else {
                match *state {
                    ActionState::Requested | ActionState::Executing => {
                        if let Some(picked_thrust) = action.thrust {
                            thrust = picked_thrust;
                        } else {
                            action.thrust = Some(thrust);
                        }

                        *state = if reached_goal {
                            // XXX probably redundant due to cancellation
                            ActionState::Success
                        } else {
                            if !crate::movement::accelerate_orb(
                                &time,
                                thrust,
                                transform.as_mut(),
                                velocity.as_mut(),
                                impulse.as_mut(),
                            ) {
                                ActionState::Executing
                            } else {
                                action.since += time.delta();
                                if action.since >= MIN_THRUST_PERIOD {
                                    ActionState::Success
                                } else {
                                    ActionState::Executing
                                }
                            }
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

/// intent to stay away from the player
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
                } else {
                    score.set(0.0);
                }
            }
        }
    }
}

/// intent to get near the player
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
                } else {
                    score.set(0.0);
                }
            }
        }
    }
}

/// low-value desire for idleness
#[derive(Clone, Component, Debug, ScorerBuilder)]
struct ExperiencingInertia;

fn moving_scorer(
    orbs: Query<&Velocity, With<Orb>>,
    mut scorers: Query<(&Actor, &mut Score), With<ExperiencingInertia>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        if let Ok(velocity) = orbs.get(*actor) {
            if velocity.angvel != 0.0 || velocity.linvel != Vec2::ZERO {
                score.set(0.1);
            } else {
                score.set(0.0);
            }
        }
    }
}

/// high-value fear of pits
#[derive(Clone, Component, Debug, ScorerBuilder)]
struct NearPit;

fn near_pit_scorer(
    pits: Res<LevelPits>,
    orbs: Query<&Transform, With<Orb>>,
    mut scorers: Query<(&Actor, &mut Score), With<NearPit>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        if let Ok(transform) = orbs.get(*actor) {
            let pit_vec = pits.nearest_pit(&transform.translation.xy());
            let pit_dist = pit_vec.length();

            debug!("pit_vec({pit_vec}) pit_dist({pit_dist})");

            if pit_dist < 256.0 * 3.0 {
                score.set(1.0);
            } else {
                score.set(0.0);
            }
        }
    }
}

pub fn plugin() -> impl Plugin {
    OpaquePlugin(|app| {
        app.add_plugins(BigBrainPlugin::new(PreUpdate))
            .add_systems(
                PreUpdate,
                (relative_move_action, halt_action).in_set(BigBrainSet::Actions),
            )
            .add_systems(
                PreUpdate,
                (moving_scorer, near_pit_scorer, flee_scorer, charge_scorer)
                    .in_set(BigBrainSet::Scorers),
            );
    })
}

pub fn spawn_intransigence(entity: &mut EntityCommands) {
    entity.insert(
        Thinker::build()
            .label("intransigence")
            .picker(Highest)
            .when(ExperiencingInertia, Halt),
    );
}

pub fn spawn_cowardice(entity: &mut EntityCommands) {
    entity.insert(
        Thinker::build()
            .label("cowardice")
            .picker(FirstToScore { threshold: 0.5 })
            .when(NearPit, RelativeMove::from(MoveType::AvoidPit))
            .otherwise(
                Thinker::build()
                    .picker(Highest)
                    .when(Flee, RelativeMove::from(MoveType::AvoidPlayer))
                    .when(ExperiencingInertia, Halt),
            ),
    );
}

pub fn spawn_malice(entity: &mut EntityCommands) {
    entity.insert(
        Thinker::build()
            .label("malice")
            .picker(FirstToScore { threshold: 0.5 })
            .when(NearPit, RelativeMove::from(MoveType::AvoidPit))
            .otherwise(
                Thinker::build()
                    .picker(Highest)
                    .when(Charge, RelativeMove::from(MoveType::ChasePlayer))
                    .when(ExperiencingInertia, Halt),
            ),
    );
}
