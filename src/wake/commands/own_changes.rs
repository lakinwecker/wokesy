use reqwest::StatusCode;
use tokio::time::Instant;

use super::{Commands, Reply, UNKNOWN_SENDER};
use crate::chat::{ChatUserId, MessageLink, silenced};
use crate::on_call::{Asked, name_in_chat, schedule_in_chat};
use crate::pagerduty::{ApiError, CustomShiftId, OverrideId, ScheduleId, ScheduleName, UserId};
use crate::people::Directory;

const NOTHING_OF_YOURS: &str = "I have no override or shift of yours on record. I only end the ones I made myself, and I forget them when I restart. Anything I made earlier still ends on time by itself, or you can remove it in PagerDuty.";
const NOTHING_TO_UNDO: &str = "Nothing to undo: I have no override or shift of mine on record. I forget them when I restart; anything I made earlier still ends on time by itself.";
const BACK_TO_THE_DEFAULTS: &str = ":leftwards_arrow_with_hook: Back to the defaults.";
const PAGERDUTY_DECIDES_AGAIN: &str = "PagerDuty's own schedules and overrides decide again.";
const BETWEEN_NAMES: &str = ", ";
const BETWEEN_FAILURES: &str = "; ";
const KINDS: [Kind; 2] = [Kind::Override, Kind::CustomShift];

#[derive(Clone, Debug)]
pub(super) struct WakeSchedule {
    pub id: ScheduleId,
    pub name: Option<ScheduleName>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Override,
    CustomShift,
}

#[derive(Clone, Debug)]
pub(super) enum Made {
    Override(OverrideId),
    CustomShift(CustomShiftId),
}

#[derive(Clone, Debug)]
pub(super) struct OwnChange {
    pub schedule: WakeSchedule,
    pub made: Made,
    pub user: UserId,
    pub asked_in: Option<MessageLink>,
    pub expires: Instant,
}

struct Ending {
    schedule: WakeSchedule,
    kind: Kind,
    user: UserId,
    failure: Option<String>,
}

impl Kind {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Override => "override",
            Self::CustomShift => "shift",
        }
    }

    const fn is_gone_after(self) -> &'static [StatusCode] {
        match self {
            Self::Override => &[StatusCode::NOT_FOUND],
            Self::CustomShift => &[StatusCode::NOT_FOUND, StatusCode::BAD_REQUEST],
        }
    }
}

impl Made {
    pub(super) const fn kind(&self) -> Kind {
        match self {
            Self::Override(_) => Kind::Override,
            Self::CustomShift(_) => Kind::CustomShift,
        }
    }

    pub(super) fn id(&self) -> &str {
        match self {
            Self::Override(id) => id,
            Self::CustomShift(id) => id,
        }
    }
}

impl WakeSchedule {
    pub(super) fn in_chat(&self) -> String {
        self.name
            .as_ref()
            .map_or_else(|| format!("`{}`", silenced(&self.id)), schedule_in_chat)
    }
}

impl Commands {
    pub(super) fn tell_who_asked(&self) {
        let asked = self
            .own_changes
            .iter()
            .filter_map(|own| {
                Some(Asked {
                    user: own.user.clone(),
                    schedule: own.schedule.id.clone(),
                    in_message: own.asked_in.clone()?,
                    until: own.expires,
                })
            })
            .collect();
        self.asked.send_replace(asked);
    }

    pub(super) fn forget_expired(&mut self) {
        let now = Instant::now();
        self.own_changes.retain(|own| own.expires > now);
    }

    pub(super) async fn stop(&mut self, sender: ChatUserId, directory: &Directory) -> Reply {
        let sender = directory.pagerduty_user_of(sender).map(|user| &user.id);
        let endings = self.end(|own| Some(&own.user) == sender).await;
        if endings.is_empty() {
            return Reply::message(NOTHING_OF_YOURS);
        }
        reply_after(&endings, stopped(&endings))
    }

    pub(super) async fn reset(&mut self, sender: ChatUserId, directory: &Directory) -> Reply {
        if directory.pagerduty_user_of(sender).is_none() {
            return Reply::message(UNKNOWN_SENDER);
        }
        let endings = self.end(|_| true).await;
        if endings.is_empty() {
            return Reply::message(NOTHING_TO_UNDO);
        }
        reply_after(&endings, back_to_the_defaults(&endings, directory))
    }

    async fn end(&mut self, chosen: impl Fn(&OwnChange) -> bool) -> Vec<Ending> {
        self.forget_expired();
        let (ending, kept): (Vec<OwnChange>, Vec<OwnChange>) =
            std::mem::take(&mut self.own_changes)
                .into_iter()
                .partition(chosen);
        self.own_changes = kept;
        self.tell_who_asked();
        let mut endings = Vec::with_capacity(ending.len());
        for own in ending {
            let kind = own.made.kind();
            let failure = match self.delete(&own).await {
                Ok(()) => {
                    tracing::info!(schedule = %own.schedule.id, kind = kind.name(), id = own.made.id(), user = %own.user, "ended what was made on a schedule");
                    None
                }
                Err(error) => {
                    tracing::warn!(schedule = %own.schedule.id, kind = kind.name(), id = own.made.id(), user = %own.user, error = %error, cause = ?std::error::Error::source(&error), "could not end what was made on a schedule");
                    Some(error.to_string())
                }
            };
            let ending = Ending {
                schedule: own.schedule.clone(),
                kind,
                user: own.user.clone(),
                failure,
            };
            if ending.failure.is_some() {
                self.own_changes.push(own);
            }
            endings.push(ending);
        }
        if !endings.is_empty() {
            self.tell_who_asked();
            self.refresh.notify_one();
        }
        endings
    }

    async fn delete(&self, own: &OwnChange) -> Result<(), ApiError> {
        let schedule = &own.schedule.id;
        let deleted = match &own.made {
            Made::Override(id) => self.pagerduty.delete_schedule_override(schedule, id).await,
            Made::CustomShift(id) => self.pagerduty.delete_custom_shift(schedule, id).await,
        };
        match deleted {
            Err(ApiError::Rejected { status, .. })
                if own.made.kind().is_gone_after().contains(&status) =>
            {
                Ok(())
            }
            answered => answered,
        }
    }
}

fn reply_after(endings: &[Ending], in_words: String) -> Reply {
    if endings.iter().all(|ending| ending.failure.is_none()) {
        Reply::acknowledgement(in_words)
    } else {
        Reply::message(in_words)
    }
}

fn stopped(endings: &[Ending]) -> String {
    let ended = KINDS.into_iter().filter_map(|kind| {
        let ended = distinct(
            endings
                .iter()
                .filter(|ending| ending.kind == kind && ending.failure.is_none())
                .map(|ending| ending.schedule.in_chat()),
        );
        (!ended.is_empty()).then(|| {
            format!(
                ":zzz: Ended your {} on {}. The schedule decides again.",
                kind.name(),
                ended.join(BETWEEN_NAMES)
            )
        })
    });
    let failed = distinct(endings.iter().filter_map(|ending| {
        let error = ending.failure.as_ref()?;
        Some(format!(
            "I couldn't end your {} on {}: {error}",
            ending.kind.name(),
            ending.schedule.in_chat()
        ))
    }));
    ended.chain(failed).collect::<Vec<_>>().join("\n")
}

fn back_to_the_defaults(endings: &[Ending], directory: &Directory) -> String {
    let mut by_schedule: Vec<(&WakeSchedule, Vec<&Ending>)> = Vec::new();
    for ending in endings {
        let listed = by_schedule
            .iter_mut()
            .find(|(schedule, _)| schedule.id == ending.schedule.id);
        match listed {
            Some((_, on_schedule)) => on_schedule.push(ending),
            None => by_schedule.push((&ending.schedule, vec![ending])),
        }
    }
    let lines = by_schedule.iter().map(|(schedule, on_schedule)| {
        format!(
            "- {}: {}",
            schedule.in_chat(),
            outcome(on_schedule, directory)
        )
    });
    std::iter::once(BACK_TO_THE_DEFAULTS.to_owned())
        .chain(lines)
        .chain(std::iter::once(PAGERDUTY_DECIDES_AGAIN.to_owned()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn outcome(on_schedule: &[&Ending], directory: &Directory) -> String {
    let name = |user: &UserId| {
        directory
            .name_of(user)
            .map_or_else(|| silenced(user), name_in_chat)
    };
    let ended = KINDS.into_iter().filter_map(|kind| {
        let ended = distinct(
            on_schedule
                .iter()
                .filter(|ending| ending.kind == kind && ending.failure.is_none())
                .map(|ending| name(&ending.user)),
        );
        (!ended.is_empty()).then(|| {
            format!(
                "ended the {} of {}.",
                kind.name(),
                ended.join(BETWEEN_NAMES)
            )
        })
    });
    let failed = distinct(on_schedule.iter().filter_map(|ending| {
        let error = ending.failure.as_ref()?;
        Some(format!(
            "I couldn't end the {} of {}: {error}",
            ending.kind.name(),
            name(&ending.user)
        ))
    }));
    let failed = (!failed.is_empty()).then(|| failed.join(BETWEEN_FAILURES));
    ended.chain(failed).collect::<Vec<_>>().join(" ")
}

fn distinct<T: PartialEq>(all: impl Iterator<Item = T>) -> Vec<T> {
    let mut distinct = Vec::new();
    for one in all {
        if !distinct.contains(&one) {
            distinct.push(one);
        }
    }
    distinct
}

#[cfg(test)]
mod tests;
