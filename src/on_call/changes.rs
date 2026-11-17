use std::collections::{BTreeMap, BTreeSet};

use super::roster::{Level, Policy, Roster, Shift};
use crate::pagerduty::{EscalationLevel, EscalationPolicyName, ScheduleRef, UserId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change<'a> {
    pub policy: &'a EscalationPolicyName,
    pub level: EscalationLevel,
    pub schedules: &'a [ScheduleRef],
    pub on_call: &'a [Shift],
    pub was_on_call: &'a [Shift],
}

pub fn changes<'a>(announced: &'a Roster, latest: &'a Roster) -> Vec<Change<'a>> {
    let mut before: BTreeMap<_, _> = levels_of(announced)
        .map(|(policy, level)| ((&policy.id, level.number), level.shifts.as_slice()))
        .collect();
    let mut changes = Vec::new();
    for (policy, level) in levels_of(latest) {
        let was_on_call = before
            .remove(&(&policy.id, level.number))
            .unwrap_or_default();
        if people(was_on_call) != people(&level.shifts) {
            changes.push(Change {
                policy: &policy.name,
                level: level.number,
                schedules: &level.schedules,
                on_call: &level.shifts,
                was_on_call,
            });
        }
    }
    for (policy, level) in levels_of(announced) {
        let gone = before.contains_key(&(&policy.id, level.number));
        if gone && !level.shifts.is_empty() {
            changes.push(Change {
                policy: &policy.name,
                level: level.number,
                schedules: &level.schedules,
                on_call: &[],
                was_on_call: &level.shifts,
            });
        }
    }
    changes
}

fn levels_of(roster: &Roster) -> impl Iterator<Item = (&Policy, &Level)> {
    roster
        .policies
        .iter()
        .flat_map(|policy| policy.levels.iter().map(move |level| (policy, level)))
}

fn people(shifts: &[Shift]) -> BTreeSet<&UserId> {
    shifts.iter().map(|shift| &shift.user.id).collect()
}

#[cfg(test)]
mod tests;
