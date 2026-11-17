use time::macros::datetime;

use super::*;
use crate::on_call::testing::{level, policy, policy_name, roster, shift, user};

fn alice() -> Shift {
    shift(
        user("PALICE1", "Alice Example"),
        Some(datetime!(2026-11-14 20:00 UTC)),
    )
}

fn bob() -> Shift {
    shift(user("PBOB001", "Bob Example"), None)
}

fn carol() -> Shift {
    shift(user("PCAROL1", "Carol Example"), None)
}

fn lichess(levels: &[&[Shift]]) -> Roster {
    let levels: Vec<Level> = (1..)
        .zip(levels)
        .map(|(number, shifts)| level(number, shifts))
        .collect();
    roster(&[policy("PLICHES", "Lichess", &levels)])
}

fn change<'a>(
    policy: &'a EscalationPolicyName,
    level: u32,
    on_call: &'a [Shift],
    was_on_call: &'a [Shift],
) -> Change<'a> {
    Change {
        policy,
        level: EscalationLevel::new(level),
        schedules: &[],
        on_call,
        was_on_call,
    }
}

#[test]
fn the_same_roster_has_no_changes() {
    let announced = lichess(&[&[alice(), bob()], &[], &[carol()]]);

    assert_eq!(changes(&announced, &announced.clone()), []);
}

#[test]
fn a_different_set_of_people_at_a_level_is_a_change_of_that_level_only() {
    let announced = lichess(&[&[alice()], &[bob()], &[carol()], &[]]);
    let latest = lichess(&[&[alice()], &[bob(), carol()], &[], &[alice()]]);

    assert_eq!(
        changes(&announced, &latest),
        [
            change(&policy_name("Lichess"), 2, &[bob(), carol()], &[bob()]),
            change(&policy_name("Lichess"), 3, &[], &[carol()]),
            change(&policy_name("Lichess"), 4, &[alice()], &[]),
        ]
    );
}

#[test]
fn the_same_people_in_another_order_are_not_a_change() {
    let announced = lichess(&[&[alice(), bob()]]);
    let latest = lichess(&[&[bob(), alice()]]);

    assert_eq!(changes(&announced, &latest), []);
}

#[test]
fn a_moved_shift_end_is_not_a_change() {
    let announced = lichess(&[&[alice(), bob()]]);
    let later = Shift {
        ends: Some(datetime!(2026-11-15 08:00 UTC)),
        ..alice()
    };
    let open_ended = Shift {
        ends: None,
        ..alice()
    };

    assert_eq!(changes(&announced, &lichess(&[&[later, bob()]])), []);
    assert_eq!(changes(&announced, &lichess(&[&[open_ended, bob()]])), []);
}

#[test]
fn a_renamed_person_or_policy_is_not_a_change() {
    let announced = lichess(&[&[bob()]]);
    let robert = shift(user("PBOB001", "Robert Example"), None);
    let latest = roster(&[policy("PLICHES", "Lichess.org", &[level(1, &[robert])])]);

    assert_eq!(changes(&announced, &latest), []);
}

#[test]
fn a_level_that_appears_changes_from_nobody_to_whoever_is_on_call_there() {
    let announced = lichess(&[&[alice()]]);
    let latest = lichess(&[&[alice()], &[bob()], &[]]);

    assert_eq!(
        changes(&announced, &latest),
        [change(&policy_name("Lichess"), 2, &[bob()], &[])]
    );
}

#[test]
fn a_level_that_disappears_changes_from_whoever_was_on_call_there_to_nobody() {
    let announced = lichess(&[&[alice()], &[bob()], &[]]);
    let latest = lichess(&[&[alice()]]);

    assert_eq!(
        changes(&announced, &latest),
        [change(&policy_name("Lichess"), 2, &[], &[bob()])]
    );
}

#[test]
fn a_policy_that_appears_or_disappears_changes_each_level_somebody_is_on_call_at() {
    let moderation = policy(
        "PMODERA",
        "Moderation",
        &[level(1, &[carol()]), level(2, &[]), level(3, &[bob()])],
    );
    let lichess = policy("PLICHES", "Lichess", &[level(1, &[alice()])]);
    let both = roster(&[lichess.clone(), moderation]);
    let only_lichess = roster(&[lichess]);

    assert_eq!(
        changes(&only_lichess, &both),
        [
            change(&policy_name("Moderation"), 1, &[carol()], &[]),
            change(&policy_name("Moderation"), 3, &[bob()], &[]),
        ]
    );
    assert_eq!(
        changes(&both, &only_lichess),
        [
            change(&policy_name("Moderation"), 1, &[], &[carol()]),
            change(&policy_name("Moderation"), 3, &[], &[bob()]),
        ]
    );
}

#[test]
fn changes_follow_the_latest_roster_and_then_come_the_levels_that_are_gone() {
    let abandoned = policy("PABANDO", "Abandoned", &[level(1, &[carol()])]);
    let announced = roster(&[
        abandoned,
        policy("PLICHES", "Lichess", &[level(1, &[alice()])]),
        policy("PMODERA", "Moderation", &[level(1, &[alice()])]),
    ]);
    let latest = roster(&[
        policy("PLICHES", "Lichess", &[level(1, &[bob()])]),
        policy("PMODERA", "Moderation", &[level(1, &[bob()])]),
    ]);

    assert_eq!(
        changes(&announced, &latest),
        [
            change(&policy_name("Lichess"), 1, &[bob()], &[alice()]),
            change(&policy_name("Moderation"), 1, &[bob()], &[alice()]),
            change(&policy_name("Abandoned"), 1, &[], &[carol()]),
        ]
    );
}
