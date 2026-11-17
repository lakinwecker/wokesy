use std::time::Duration;

use time::OffsetDateTime;
use tokio::sync::watch;
use tokio::time::Instant;

use super::asked::Asked;
use super::changes::changes;
use super::rendering::{on_call_changed, on_call_now};
use super::roster::Roster;
use crate::chat::{Conversation, Messenger, OutgoingMessage};
use crate::people::Directory;

const RETRY_DELIVERY_AFTER: Duration = Duration::from_secs(30);

pub async fn keep_announcing<M: Messenger>(
    messenger: M,
    to: Conversation,
    mut roster: watch::Receiver<Option<Roster>>,
    mut directory: watch::Receiver<Option<Directory>>,
    asked: watch::Receiver<Vec<Asked>>,
) {
    if directory.wait_for(Option::is_some).await.is_err() {
        return;
    }
    let mut announced: Option<Roster> = None;
    loop {
        if let Some((latest, announcement)) =
            latest(&mut roster, &directory, &asked, announced.as_ref())
        {
            if let Some(announcement) = announcement
                && let Err(error) = messenger.send(&to, &announcement).await
            {
                tracing::warn!(error = %error, cause = %error.cause, retry_in = ?RETRY_DELIVERY_AFTER, "on-call announcement delivery failed");
                tokio::time::sleep(RETRY_DELIVERY_AFTER).await;
                continue;
            }
            announced = Some(latest);
        }
        if roster.changed().await.is_err() {
            return;
        }
    }
}

fn latest(
    roster: &mut watch::Receiver<Option<Roster>>,
    directory: &watch::Receiver<Option<Directory>>,
    asked: &watch::Receiver<Vec<Asked>>,
    announced: Option<&Roster>,
) -> Option<(Roster, Option<OutgoingMessage>)> {
    let latest = roster.borrow_and_update().clone()?;
    let now = Instant::now();
    let asked: Vec<Asked> = asked
        .borrow()
        .iter()
        .filter(|asked| asked.until > now)
        .cloned()
        .collect();
    let announcement = announcement(announced, &latest, directory.borrow().as_ref()?, &asked);
    Some((latest, announcement))
}

fn announcement(
    announced: Option<&Roster>,
    latest: &Roster,
    directory: &Directory,
    asked: &[Asked],
) -> Option<OutgoingMessage> {
    let now = OffsetDateTime::now_utc();
    let Some(announced) = announced else {
        return Some(OutgoingMessage::new(on_call_now(latest, directory, now)));
    };
    let changes = changes(announced, latest);
    if changes.is_empty() {
        return None;
    }
    Some(OutgoingMessage::new(on_call_changed(
        &changes, latest, directory, asked, now,
    )))
}

#[cfg(test)]
mod tests;
