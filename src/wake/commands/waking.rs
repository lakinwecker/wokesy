use std::collections::BTreeSet;
use std::time::Duration;

use humantime::format_duration;
use time::OffsetDateTime;
use tokio::time::Instant;

use super::backup::backup;
use super::own_changes::{Kind, Made, OwnChange, WakeSchedule};
use super::{Commands, FIRST_LINE, Reply, UNKNOWN_SENDER};
use crate::chat::{ChatUserId, IncomingMessage, in_words, time_tag};
use crate::on_call::{Level, Policy, Roster, name_in_chat, policy_in_chat};
use crate::pagerduty::{self, ApiError, ScheduleId, ScheduleName, UserId};
use crate::people::Directory;

const FOR_HOW_LONG: &str = "For how long? For example: `wake me for 4 hours`.";
const ON_NO_SCHEDULE: &str = ":warning: I couldn't put you on any schedule.";
const HANDED_BACK: &str = "PagerDuty hands it back to the schedule by itself when the time is up. Send `stop waking me` to end it sooner.";
const SHIFT_ENDS: &str = "PagerDuty ends the shift by itself when the time is up. Send `stop waking me` to end it sooner.";
const ON_IT: &str = "you are on it";
const ON_IT_ALONGSIDE: &str = "you are on it, alongside whoever the schedule has";
const NOT_AT_THE_FIRST_LINE: &str =
    "but it is not at level 1 of any escalation policy in use, so alerts may not reach you first.";
const BETWEEN_NAMES: &str = ", ";

struct Stint {
    starts: OffsetDateTime,
    ends: OffsetDateTime,
    expires: Instant,
}

impl Stint {
    fn starting_now(lasting: Duration) -> Option<Self> {
        let starts = OffsetDateTime::now_utc();
        Some(Self {
            starts,
            ends: starts.checked_add(lasting.try_into().ok()?)?,
            expires: Instant::now().checked_add(lasting)?,
        })
    }
}

impl Commands {
    pub(super) async fn wake(
        &mut self,
        message: &IncomingMessage,
        lasting: Option<Duration>,
        roster: &Roster,
        directory: &Directory,
    ) -> Reply {
        let sender = message.sender;
        let Some(user) = directory.pagerduty_user_of(sender) else {
            return Reply::message(UNKNOWN_SENDER);
        };
        let Some(lasting) = lasting.filter(|lasting| !lasting.is_zero()) else {
            return Reply::message(FOR_HOW_LONG);
        };
        let allowed = (lasting <= self.config.longest)
            .then(|| Stint::starting_now(lasting))
            .flatten();
        let Some(stint) = allowed else {
            return Reply::message(format!(
                "That is longer than I'm allowed to set ({}). Ask for less, or change the schedule in PagerDuty.",
                format_duration(self.config.longest)
            ));
        };
        self.forget_expired();
        let mut lines = Vec::new();
        let mut made_on_some_schedule = Vec::new();
        for id in self.config.schedules.ids() {
            let schedule = WakeSchedule {
                id: id.clone(),
                name: name_of(id, roster),
            };
            let (kind, made) =
                put_on(&self.pagerduty, &mut self.shift_based, id, &user.id, &stint).await;
            let outcome = match made {
                Ok(made) => {
                    tracing::info!(schedule = %id, kind = kind.name(), id = made.id(), user = %user.id, "put a person on a schedule");
                    self.own_changes.push(OwnChange {
                        schedule: schedule.clone(),
                        made,
                        user: user.id.clone(),
                        asked_in: message.link(),
                        expires: stint.expires,
                    });
                    self.tell_who_asked();
                    made_on_some_schedule.push(kind);
                    on_it(id, roster, kind)
                }
                Err(error) => {
                    tracing::warn!(schedule = %id, kind = kind.name(), user = %user.id, error = %error, cause = ?std::error::Error::source(&error), "could not put a person on a schedule");
                    could_not(kind, &error)
                }
            };
            lines.push(format!("- {}: {outcome}", schedule.in_chat()));
        }
        self.refresh.notify_one();
        let schedules = self.config.schedules.ids();
        now_on_call(
            sender,
            &user.id,
            lasting,
            schedules,
            &made_on_some_schedule,
            roster,
        )
        .unwrap_or_else(|| Reply::message(woken(stint.ends, &made_on_some_schedule, lines)))
    }
}

fn now_on_call(
    sender: ChatUserId,
    user: &UserId,
    lasting: Duration,
    schedules: &[ScheduleId],
    made: &[Kind],
    roster: &Roster,
) -> Option<Reply> {
    if made.len() != schedules.len() {
        return None;
    }
    let [policy] = roster.policies.as_slice() else {
        return None;
    };
    let first_line = policy
        .levels
        .iter()
        .find(|level| level.number.into_inner() == FIRST_LINE)?;
    let on_the_first_line = |schedule: &ScheduleId| {
        first_line
            .schedules
            .iter()
            .any(|target| target.id == *schedule)
    };
    if !schedules.iter().all(on_the_first_line) {
        return None;
    }
    let others = others_on(first_line, user, schedules, made)?;
    let alongside = match others {
        0 => String::new(),
        1 => format!(", together with 1 other at level {FIRST_LINE}"),
        others => format!(", together with {others} others at level {FIRST_LINE}"),
    };
    let on_call = format!(
        ":alarm_clock: {} is now on call for {}{alongside}.",
        sender.silent_mention(),
        in_words(lasting)
    );
    let behind = policy
        .levels
        .iter()
        .filter(|level| level.number > first_line.number);
    let in_one_line = match backup(behind) {
        Some(backup) => format!("{on_call} {backup}"),
        None => on_call,
    };
    Some(if others == 0 {
        Reply::acknowledgement(in_one_line)
    } else {
        Reply::message(in_one_line)
    })
}

fn others_on(
    first_line: &Level,
    user: &UserId,
    schedules: &[ScheduleId],
    made: &[Kind],
) -> Option<usize> {
    let all = |kind: Kind| made.iter().all(|made| *made == kind);
    if all(Kind::CustomShift) {
        let on_call = first_line.shifts.iter();
        return Some(on_call.filter(|shift| shift.user.id != *user).count());
    }
    let everyone_scheduled_is_replaced = all(Kind::Override)
        && first_line
            .schedules
            .iter()
            .all(|target| schedules.contains(&target.id));
    everyone_scheduled_is_replaced.then(|| {
        let paged_directly = first_line.people.iter();
        paged_directly.filter(|person| person.id != *user).count()
    })
}

async fn put_on(
    pagerduty: &pagerduty::Api,
    shift_based: &mut BTreeSet<ScheduleId>,
    schedule: &ScheduleId,
    user: &UserId,
    stint: &Stint,
) -> (Kind, Result<Made, ApiError>) {
    if !shift_based.contains(schedule) {
        let overridden = pagerduty
            .create_schedule_override(schedule, user, stint.starts, stint.ends)
            .await;
        if !matches!(overridden, Err(ApiError::ShiftBasedSchedule { .. })) {
            return (Kind::Override, overridden.map(Made::Override));
        }
        tracing::info!(schedule = %schedule, "the schedule is shift-based, so it gets a custom shift instead of an override");
        shift_based.insert(schedule.clone());
    }
    let added = pagerduty
        .create_custom_shift(schedule, user, stint.starts, stint.ends)
        .await;
    (Kind::CustomShift, added.map(Made::CustomShift))
}

fn could_not(kind: Kind, error: &ApiError) -> String {
    if !certainly_refused(error) {
        return format!(
            "I couldn't confirm the {}: {error}. It may exist; check PagerDuty.",
            kind.name()
        );
    }
    match kind {
        Kind::Override => format!("I couldn't override it: {error}"),
        Kind::CustomShift => format!("I couldn't add a shift to it: {error}"),
    }
}

fn certainly_refused(error: &ApiError) -> bool {
    matches!(
        error,
        ApiError::Rejected { status, .. } | ApiError::ShiftBasedSchedule { status, .. }
            if status.is_client_error()
    )
}

fn woken(until: OffsetDateTime, made: &[Kind], lines: Vec<String>) -> String {
    let (heading, ending) = if made.is_empty() {
        (ON_NO_SCHEDULE.to_owned(), None)
    } else {
        let heading = format!(
            ":alarm_clock: I'll wake you for anything until {}.",
            time_tag(until)
        );
        let ending = if made.contains(&Kind::Override) {
            HANDED_BACK
        } else {
            SHIFT_ENDS
        };
        (heading, Some(ending.to_owned()))
    };
    std::iter::once(heading)
        .chain(lines)
        .chain(ending)
        .collect::<Vec<_>>()
        .join("\n")
}

fn name_of(schedule: &ScheduleId, roster: &Roster) -> Option<ScheduleName> {
    roster
        .policies
        .iter()
        .flat_map(|policy| &policy.levels)
        .flat_map(|level| &level.schedules)
        .find(|known| known.id == *schedule)
        .map(|known| known.name.clone())
}

fn on_it(schedule: &ScheduleId, roster: &Roster, kind: Kind) -> String {
    let on_it = match kind {
        Kind::Override => ON_IT,
        Kind::CustomShift => ON_IT_ALONGSIDE,
    };
    let first_lines: Vec<(&Policy, &Level)> = roster
        .policies
        .iter()
        .filter_map(|policy| {
            let first_line = policy
                .levels
                .iter()
                .find(|level| level.number.into_inner() == FIRST_LINE)?;
            first_line
                .schedules
                .iter()
                .any(|target| target.id == *schedule)
                .then_some((policy, first_line))
        })
        .collect();
    if first_lines.is_empty() {
        return format!("{on_it}, {NOT_AT_THE_FIRST_LINE}");
    }
    let policies: Vec<String> = first_lines
        .iter()
        .map(|(policy, _)| policy_in_chat(&policy.name))
        .collect();
    let mut paged_directly: Vec<&UserId> = Vec::new();
    let mut names = Vec::new();
    for person in first_lines.iter().flat_map(|(_, level)| &level.people) {
        if !paged_directly.contains(&&person.id) {
            paged_directly.push(&person.id);
            names.push(name_in_chat(&person.name));
        }
    }
    let level_1_of = format!(
        "{on_it}. It is level 1 of {}.",
        policies.join(BETWEEN_NAMES)
    );
    if names.is_empty() {
        level_1_of
    } else {
        format!(
            "{level_1_of} Level 1 also pages {} directly.",
            names.join(BETWEEN_NAMES)
        )
    }
}

#[cfg(test)]
mod tests;
