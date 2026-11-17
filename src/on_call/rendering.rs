use time::OffsetDateTime;

use super::asked::Asked;
use super::changes::Change;
use super::roster::{Policy, Roster, Shift};
use crate::chat::{ChatUserId, MessageLink, in_one_unit, silenced, time_tag};
use crate::pagerduty::{EscalationLevel, EscalationPolicyName, ScheduleName, UserRef};
use crate::people::{Directory, Name};

const ON_CALL_NOW: &str = ":pager: **On call now:**";
const ON_CALL_CHANGED: &str = ":pager: **On call changed:**";
const NO_POLICY_IN_USE: &str =
    ":pager: **On call now**\nNo escalation policy is used by a service, so nobody is on call.";
const NOBODY_TO_ALERT: &str = ":warning: nobody";
const NOBODY: &str = "nobody";
const SCHEDULE: &str = "schedule";
const CHANGES: &str = "changes";
const BETWEEN_PEOPLE: &str = ", ";
const BETWEEN_LEVELS: &str = "\n\n";

/// ```
/// use wokesy::on_call::policy_in_chat;
/// use wokesy::pagerduty::EscalationPolicyName;
///
/// let name = |name: &str| EscalationPolicyName::try_new(name).unwrap();
///
/// assert_eq!(policy_in_chat(&name("Lichess")), "**Lichess**");
/// assert_eq!(policy_in_chat(&name("Ops @**all**")), "**Ops @_**all****");
/// ```
#[must_use]
pub fn policy_in_chat(name: &EscalationPolicyName) -> String {
    bold(name)
}

/// ```
/// use wokesy::on_call::schedule_in_chat;
/// use wokesy::pagerduty::ScheduleName;
///
/// let name = |name: &str| ScheduleName::try_new(name).unwrap();
///
/// assert_eq!(schedule_in_chat(&name("Primary rota")), "**Primary rota**");
/// assert_eq!(schedule_in_chat(&name("Rota of @*sysadmins*")), "**Rota of @_*sysadmins***");
/// ```
#[must_use]
pub fn schedule_in_chat(name: &ScheduleName) -> String {
    bold(name)
}

/// ```
/// use wokesy::on_call::name_in_chat;
/// use wokesy::people::Name;
///
/// let name = |name: &str| Name::try_new(name).unwrap();
///
/// assert_eq!(name_in_chat(&name("Carol Example")), "Carol Example");
/// assert_eq!(name_in_chat(&name("@**all**")), "@_**all**");
/// ```
#[must_use]
pub fn name_in_chat(name: &Name) -> String {
    silenced(name)
}

/// ```
/// use wokesy::on_call::person_in_chat;
/// use wokesy::pagerduty::{User, UserId, UserRef};
/// use wokesy::people::{Directory, Email, Links, Name};
///
/// let user = |id: &str, name: &str| User {
///     id: UserId::try_new(id).unwrap(),
///     name: Name::try_new(name).unwrap(),
///     email: Email::try_new(format!("{id}@example.com")).unwrap(),
/// };
/// let on_call = |user: &User| UserRef { id: user.id.clone(), name: user.name.clone() };
/// let (alice, everyone) = (user("PALICE1", "Alice Example"), user("PEVERY1", "@**all**"));
/// let links: Links = "PALICE1@example.com=12".parse().unwrap();
/// let directory = Directory::matching(&[alice.clone(), everyone.clone()], &[], &links);
///
/// assert_eq!(person_in_chat(&on_call(&alice), &directory), "@_**|12**");
/// assert_eq!(person_in_chat(&on_call(&everyone), &directory), "@_**all**");
/// ```
#[must_use]
pub fn person_in_chat(user: &UserRef, directory: &Directory) -> String {
    directory
        .chat_user_of(&user.id)
        .map_or_else(|| name_in_chat(&user.name), ChatUserId::silent_mention)
}

fn bold(name: &str) -> String {
    format!("**{}**", silenced(name))
}

/// ```
/// use time::OffsetDateTime;
/// use wokesy::on_call::{ChosenPolicies, Roster, on_call_now};
/// use wokesy::people::{Directory, Links};
///
/// let nobody_to_call = Roster::new(vec![], vec![], &ChosenPolicies::default());
/// let directory = Directory::matching(&[], &[], &Links::default());
///
/// assert_eq!(
///     on_call_now(&nobody_to_call, &directory, OffsetDateTime::now_utc()),
///     ":pager: **On call now**\nNo escalation policy is used by a service, so nobody is on call."
/// );
/// ```
#[must_use]
pub fn on_call_now(roster: &Roster, directory: &Directory, now: OffsetDateTime) -> String {
    if roster.policies.is_empty() {
        return NO_POLICY_IN_USE.to_owned();
    }
    let levels = roster.policies.iter().flat_map(|policy| {
        policy.levels.iter().map(|level| {
            under_its_heading(
                &policy.name,
                level.number,
                on_call(&level.shifts, directory),
            )
        })
    });
    format!(
        "{ON_CALL_NOW} {}\n\n{}",
        alerted_first(roster, directory, now),
        hidden_under(SCHEDULE, levels)
    )
}

pub fn on_call_changed(
    changes: &[Change<'_>],
    latest: &Roster,
    directory: &Directory,
    asked: &[Asked],
    now: OffsetDateTime,
) -> String {
    let levels = changes.iter().map(|change| {
        let was = format!("was: {}", previously_on_call(change.was_on_call, directory));
        let now_on_call = change.on_call.iter().map(|shift| {
            let on_call = on_call_until(shift, directory);
            match asked_in(shift, change, asked) {
                Some(message) => format!("{on_call}, from {}", message.in_chat()),
                None => on_call,
            }
        });
        let lines = if change.on_call.is_empty() {
            vec![NOBODY.to_owned()]
        } else {
            now_on_call.collect()
        };
        let lines = lines.into_iter().chain(std::iter::once(was));
        under_its_heading(change.policy, change.level, lines)
    });
    format!(
        "{ON_CALL_CHANGED} {}\n\n{}",
        alerted_first(latest, directory, now),
        hidden_under(CHANGES, levels)
    )
}

fn alerted_first(roster: &Roster, directory: &Directory, now: OffsetDateTime) -> String {
    let mut alerted: Vec<&Shift> = Vec::new();
    let first_lines = roster.policies.iter().filter_map(Policy::alerted_first);
    for shift in first_lines.flat_map(|level| &level.shifts) {
        if alerted.iter().all(|listed| listed.user.id != shift.user.id) {
            alerted.push(shift);
        }
    }
    if alerted.is_empty() {
        return NOBODY_TO_ALERT.to_owned();
    }
    alerted
        .into_iter()
        .map(|shift| {
            let person = person_in_chat(&shift.user, directory);
            match shift.ends {
                Some(ends) => {
                    let left = (ends - now).try_into().unwrap_or_default();
                    format!("{person} for {}", in_one_unit(left))
                }
                None => person,
            }
        })
        .collect::<Vec<_>>()
        .join(BETWEEN_PEOPLE)
}

fn hidden_under(header: &str, levels: impl IntoIterator<Item = String>) -> String {
    let levels = levels.into_iter().collect::<Vec<_>>().join(BETWEEN_LEVELS);
    format!("```spoiler {header}\n{levels}\n```")
}

fn under_its_heading(
    policy: &EscalationPolicyName,
    level: EscalationLevel,
    lines: impl IntoIterator<Item = String>,
) -> String {
    let heading = bold(&format!("{} · Level {level}", policy.as_str()));
    std::iter::once(heading)
        .chain(lines.into_iter().map(|line| format!("- {line}")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn on_call(shifts: &[Shift], directory: &Directory) -> Vec<String> {
    if shifts.is_empty() {
        return vec![NOBODY.to_owned()];
    }
    shifts
        .iter()
        .map(|shift| on_call_until(shift, directory))
        .collect()
}

fn on_call_until(shift: &Shift, directory: &Directory) -> String {
    let person = person_in_chat(&shift.user, directory);
    match shift.ends {
        Some(ends) => format!("{person} until {}", time_tag(ends)),
        None => person,
    }
}

fn asked_in<'a>(shift: &Shift, change: &Change<'_>, asked: &'a [Asked]) -> Option<&'a MessageLink> {
    let was_on_call = change
        .was_on_call
        .iter()
        .any(|was| was.user.id == shift.user.id);
    if was_on_call {
        return None;
    }
    asked
        .iter()
        .rev()
        .find(|asked| {
            asked.user == shift.user.id
                && change
                    .schedules
                    .iter()
                    .any(|schedule| schedule.id == asked.schedule)
        })
        .map(|asked| &asked.in_message)
}

fn previously_on_call(shifts: &[Shift], directory: &Directory) -> String {
    if shifts.is_empty() {
        return NOBODY.to_owned();
    }
    shifts
        .iter()
        .map(|shift| person_in_chat(&shift.user, directory))
        .collect::<Vec<_>>()
        .join(BETWEEN_PEOPLE)
}

#[cfg(test)]
mod tests;
