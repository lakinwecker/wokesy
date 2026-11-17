use time::macros::datetime;

use super::*;
use crate::on_call::changes::changes;
use tokio::time::Instant;

use crate::chat::{ChannelName, MessageId, TopicName};
use crate::on_call::Level;
use crate::on_call::testing::{directory, level, policy, roster, schedule, shift, user};
use crate::pagerduty::ScheduleId;

const NOW: OffsetDateTime = datetime!(2026-11-14 16:00 UTC);

fn alice() -> UserRef {
    user("PALICE1", "Alice Example")
}

fn bob() -> UserRef {
    user("PBOB001", "Bob Example")
}

fn carol() -> UserRef {
    user("PCAROL1", "Carol Example")
}

fn alice_and_bob_in_chat() -> Directory {
    directory(&[(&alice(), 12), (&bob(), 13)])
}

fn until_the_evening(user: UserRef) -> Shift {
    shift(user, Some(datetime!(2026-11-14 21:00 +1)))
}

fn open_ended(user: UserRef) -> Shift {
    shift(user, None)
}

fn lichess(first_line: &[Shift]) -> Roster {
    roster(&[policy("PLICHES", "Lichess", &[level(1, first_line)])])
}

fn now(roster: &Roster) -> String {
    on_call_now(roster, &alice_and_bob_in_chat(), NOW)
}

fn changed(announced: &Roster, latest: &Roster) -> String {
    on_call_changed(
        &changes(announced, latest),
        latest,
        &alice_and_bob_in_chat(),
        &[],
        NOW,
    )
}

#[test]
fn on_call_now_is_a_line_of_who_is_alerted_first_and_a_hidden_schedule_of_every_level() {
    let roster = roster(&[policy(
        "PDEFAUL",
        "Default",
        &[
            level(
                1,
                &[
                    until_the_evening(alice()),
                    shift(bob(), Some(datetime!(2026-11-15 10:00 UTC))),
                    shift(carol(), Some(datetime!(2026-11-14 18:00 UTC))),
                ],
            ),
            level(
                2,
                &[
                    shift(alice(), Some(datetime!(2027-02-25 18:14:36 UTC))),
                    open_ended(carol()),
                ],
            ),
        ],
    )]);

    assert_eq!(
        now(&roster),
        "\
:pager: **On call now:** @_**|12** for 4 hours, @_**|13** for 18 hours, Carol Example for 2 hours

```spoiler schedule
**Default · Level 1**
- @_**|12** until <time:2026-11-14T20:00:00Z>
- @_**|13** until <time:2026-11-15T10:00:00Z>
- Carol Example until <time:2026-11-14T18:00:00Z>

**Default · Level 2**
- @_**|12** until <time:2027-02-25T18:14:36Z>
- Carol Example
```"
    );
}

#[test]
fn the_first_level_that_has_anybody_is_alerted_first_and_a_level_with_nobody_says_so() {
    let roster = roster(&[policy(
        "PLICHES",
        "Lichess",
        &[
            level(1, &[]),
            level(2, &[open_ended(alice()), until_the_evening(carol())]),
            level(3, &[open_ended(bob())]),
            level(4, &[]),
        ],
    )]);

    assert_eq!(
        now(&roster),
        "\
:pager: **On call now:** @_**|12**, Carol Example for 4 hours

```spoiler schedule
**Lichess · Level 1**
- nobody

**Lichess · Level 2**
- @_**|12**
- Carol Example until <time:2026-11-14T20:00:00Z>

**Lichess · Level 3**
- @_**|13**

**Lichess · Level 4**
- nobody
```"
    );
}

#[test]
fn with_several_policies_the_first_level_with_anybody_of_each_is_alerted_and_a_person_is_named_once()
 {
    let roster = roster(&[
        policy(
            "PLICHES",
            "Lichess",
            &[
                level(1, &[until_the_evening(alice())]),
                level(2, &[open_ended(bob())]),
            ],
        ),
        policy(
            "PMODERA",
            "Moderation",
            &[
                level(1, &[]),
                level(2, &[open_ended(alice()), open_ended(carol())]),
            ],
        ),
        policy("PSPARE1", "Spare", &[level(1, &[])]),
    ]);

    assert_eq!(
        now(&roster),
        "\
:pager: **On call now:** @_**|12** for 4 hours, Carol Example

```spoiler schedule
**Lichess · Level 1**
- @_**|12** until <time:2026-11-14T20:00:00Z>

**Lichess · Level 2**
- @_**|13**

**Moderation · Level 1**
- nobody

**Moderation · Level 2**
- @_**|12**
- Carol Example

**Spare · Level 1**
- nobody
```"
    );
}

#[test]
fn with_nobody_on_call_at_any_level_the_first_line_warns() {
    let roster = roster(&[policy(
        "PLICHES",
        "Lichess",
        &[level(1, &[]), level(2, &[])],
    )]);

    assert_eq!(
        now(&roster),
        "\
:pager: **On call now:** :warning: nobody

```spoiler schedule
**Lichess · Level 1**
- nobody

**Lichess · Level 2**
- nobody
```"
    );
}

#[test]
fn the_time_left_is_counted_from_the_moment_given_in_one_unit_and_never_less_than_a_minute() {
    let roster = lichess(&[until_the_evening(alice())]);
    let left_at = |now: OffsetDateTime| {
        let said = on_call_now(&roster, &alice_and_bob_in_chat(), now);
        said.lines().next().unwrap().to_owned()
    };

    for (now, left) in [
        (datetime!(2026-09-01 20:00 UTC), "2 months"),
        (datetime!(2026-11-12 19:00 UTC), "2 days"),
        (datetime!(2026-11-14 16:00 UTC), "4 hours"),
        (datetime!(2026-11-14 16:01 UTC), "4 hours"),
        (datetime!(2026-11-14 16:02 UTC), "3 hours"),
        (datetime!(2026-11-14 19:16 UTC), "45 minutes"),
        (datetime!(2026-11-14 19:59:50 UTC), "1 minute"),
        (datetime!(2026-11-14 20:00 UTC), "1 minute"),
        (datetime!(2026-11-14 23:00 UTC), "1 minute"),
    ] {
        assert_eq!(
            left_at(now),
            format!(":pager: **On call now:** @_**|12** for {left}"),
            "{now}"
        );
    }
}

#[test]
fn without_a_policy_in_use_nobody_is_on_call() {
    assert_eq!(
        now(&roster(&[])),
        "\
:pager: **On call now**
No escalation policy is used by a service, so nobody is on call."
    );
}

#[test]
fn nobody_is_named_with_a_mention_that_notifies_them() {
    let everyone = [
        until_the_evening(alice()),
        open_ended(bob()),
        open_ended(carol()),
    ];
    let now = now(&lichess(&everyone));
    let change = changed(&lichess(&everyone), &lichess(&[]));

    assert!(!now.contains("@**"), "{now}");
    assert!(!change.contains("@**"), "{change}");
}

#[test]
fn a_name_from_pagerduty_that_holds_a_mention_cannot_notify() {
    let everyone = user("PEVERY1", "@**all**");
    let group = user("PGROUP1", "x @*sysadmins*");
    let named_to_ping = |shifts: &[Shift]| {
        roster(&[policy(
            "PLICHES",
            "Ops @**Carol Example**",
            &[level(1, shifts)],
        )])
    };
    let on_call = named_to_ping(&[until_the_evening(everyone), open_ended(group)]);

    assert_eq!(
        now(&on_call),
        "\
:pager: **On call now:** @_**all** for 4 hours, x @_*sysadmins*

```spoiler schedule
**Ops @_**Carol Example** · Level 1**
- @_**all** until <time:2026-11-14T20:00:00Z>
- x @_*sysadmins*
```"
    );
    assert_eq!(
        changed(&on_call, &named_to_ping(&[])),
        "\
:pager: **On call changed:** :warning: nobody

```spoiler changes
**Ops @_**Carol Example** · Level 1**
\\- @_**all**
\\- x @_*sysadmins*
\\+ nobody
```"
    );
}

#[test]
fn a_change_is_a_line_of_who_is_alerted_first_now_and_the_hidden_levels_that_changed() {
    let before = |second_line: &[Shift]| {
        roster(&[policy(
            "PLICHES",
            "Lichess",
            &[
                level(1, &[until_the_evening(alice())]),
                level(2, second_line),
            ],
        )])
    };
    let announced = before(&[until_the_evening(bob())]);
    let latest = before(&[until_the_evening(bob()), open_ended(carol())]);

    assert_eq!(
        changed(&announced, &latest),
        "\
:pager: **On call changed:** @_**|12** for 4 hours

```spoiler changes
**Lichess · Level 2**
@_**|13** until <time:2026-11-14T20:00:00Z>
\\+ Carol Example
```"
    );
}

#[test]
fn a_change_marks_who_left_with_a_minus_and_who_joined_with_a_plus() {
    let announced = lichess(&[
        until_the_evening(alice()),
        until_the_evening(bob()),
        until_the_evening(carol()),
    ]);
    let latest = lichess(&[open_ended(carol())]);

    assert_eq!(
        changed(&announced, &latest),
        "\
:pager: **On call changed:** Carol Example

```spoiler changes
**Lichess · Level 1**
\\- @_**|12**
\\- @_**|13**
Carol Example
```"
    );
}

#[test]
fn a_change_from_nobody_and_a_change_to_nobody_say_so() {
    let announced = roster(&[policy(
        "PLICHES",
        "Lichess",
        &[level(1, &[]), level(2, &[open_ended(bob())])],
    )]);
    let latest = roster(&[policy(
        "PLICHES",
        "Lichess",
        &[level(1, &[until_the_evening(alice())]), level(2, &[])],
    )]);

    assert_eq!(
        changed(&announced, &latest),
        "\
:pager: **On call changed:** @_**|12** for 4 hours

```spoiler changes
**Lichess · Level 1**
\\- nobody
\\+ @_**|12** until <time:2026-11-14T20:00:00Z>

**Lichess · Level 2**
\\- @_**|13**
\\+ nobody
```"
    );
}

fn asked(user: &UserRef, schedule: &str, message: u64) -> Asked {
    Asked {
        user: user.id.clone(),
        schedule: ScheduleId::try_new(schedule).unwrap(),
        in_message: MessageLink {
            channel: ChannelName::try_new("sysadmin").unwrap(),
            topic: TopicName::try_new("incident").unwrap(),
            id: MessageId(message),
        },
        until: Instant::now(),
    }
}

#[test]
fn a_person_newly_on_call_on_a_schedule_they_asked_for_is_linked_to_the_message_that_asked_above_the_spoiler()
 {
    let on_the_primary_rota = |shifts: &[Shift]| Level {
        schedules: vec![schedule("PPRIMAR", "Primary rota")],
        ..level(1, shifts)
    };
    let announced = roster(&[policy(
        "PLICHES",
        "Lichess",
        &[on_the_primary_rota(&[open_ended(bob())]), level(2, &[])],
    )]);
    let latest = roster(&[policy(
        "PLICHES",
        "Lichess",
        &[
            on_the_primary_rota(&[until_the_evening(alice()), open_ended(bob())]),
            level(2, &[open_ended(carol())]),
        ],
    )]);
    let asked = [
        asked(&alice(), "PPRIMAR", 41),
        asked(&alice(), "PPRIMAR", 42),
        asked(&bob(), "PPRIMAR", 43),
        asked(&carol(), "PPRIMAR", 44),
    ];

    assert_eq!(
        on_call_changed(
            &changes(&announced, &latest),
            &latest,
            &alice_and_bob_in_chat(),
            &asked,
            NOW,
        ),
        "\
:pager: **On call changed:** @_**|12** for 4 hours, @_**|13**
#**sysadmin>incident@42**

```spoiler changes
**Lichess · Level 1**
\\+ @_**|12** until <time:2026-11-14T20:00:00Z>
@_**|13**

**Lichess · Level 2**
\\- nobody
\\+ Carol Example
```"
    );
}
