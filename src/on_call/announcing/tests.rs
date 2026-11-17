use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};

use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use super::*;
use crate::chat::{ChannelName, DeliveryError, MessageId, MessageLink, TopicName, time_tag};
use crate::on_call::Level;
use crate::on_call::Shift;
use crate::on_call::testing::{directory, level, policy, roster, schedule, shift, user};
use crate::pagerduty::ScheduleId;

const PATIENCE: Duration = Duration::from_mins(1);
const A_LONG_TIME: Duration = Duration::from_hours(1);
const FOUR_AND_A_HALF_HOURS: Duration = Duration::from_mins(270);
const HALF_A_DAY: Duration = Duration::from_hours(12);

static THIS_EVENING: LazyLock<OffsetDateTime> =
    LazyLock::new(|| OffsetDateTime::now_utc() + FOUR_AND_A_HALF_HOURS);

#[derive(Clone, Debug)]
struct RecordingMessenger {
    attempts: mpsc::UnboundedSender<Attempt>,
    unreachable: Arc<AtomicBool>,
}

#[derive(Debug, PartialEq, Eq)]
struct Attempt {
    to: Conversation,
    markdown: String,
    delivered: bool,
}

#[derive(Debug, thiserror::Error)]
#[error("unreachable")]
struct Unreachable;

impl Messenger for RecordingMessenger {
    fn send(
        &self,
        conversation: &Conversation,
        message: &OutgoingMessage,
    ) -> impl Future<Output = Result<(), DeliveryError>> + Send {
        let delivered = !self.unreachable.load(Ordering::SeqCst);
        self.attempts
            .send(Attempt {
                to: conversation.clone(),
                markdown: message.markdown().to_owned(),
                delivered,
            })
            .unwrap();
        std::future::ready(if delivered {
            Ok(())
        } else {
            Err(DeliveryError {
                conversation: conversation.clone(),
                cause: Box::new(Unreachable),
            })
        })
    }
}

fn on_call_topic() -> Conversation {
    Conversation::Topic {
        channel: ChannelName::try_new("sysadmin").unwrap(),
        topic: TopicName::try_new("on call").unwrap(),
    }
}

fn alice() -> Shift {
    shift(user("PALICE1", "Alice Example"), Some(*THIS_EVENING))
}

fn this_evening() -> String {
    time_tag(*THIS_EVENING)
}

fn first_announcement_of(roster: &Roster) -> String {
    on_call_now(roster, &alice_and_bob_in_chat(), OffsetDateTime::now_utc())
}

fn bob() -> Shift {
    shift(user("PBOB001", "Bob Example"), None)
}

fn carol() -> Shift {
    shift(user("PCAROL1", "Carol Example"), None)
}

fn lichess(first_line: &[Shift], second_line: &[Shift]) -> Roster {
    roster(&[policy(
        "PLICHES",
        "Lichess",
        &[level(1, first_line), level(2, second_line)],
    )])
}

fn alice_and_bob_in_chat() -> Directory {
    directory(&[(&alice().user, 12), (&bob().user, 13)])
}

struct Announcing {
    task: JoinHandle<()>,
    roster: watch::Sender<Option<Roster>>,
    directory: watch::Sender<Option<Directory>>,
    asked: watch::Sender<Vec<Asked>>,
    attempts: mpsc::UnboundedReceiver<Attempt>,
    unreachable: Arc<AtomicBool>,
}

impl Announcing {
    fn start(roster: Option<Roster>, directory: Option<Directory>) -> Self {
        let (attempted, attempts) = mpsc::unbounded_channel();
        let unreachable = Arc::new(AtomicBool::new(false));
        let messenger = RecordingMessenger {
            attempts: attempted,
            unreachable: Arc::clone(&unreachable),
        };
        let (roster, rosters) = watch::channel(roster);
        let (directory, directories) = watch::channel(directory);
        let (asked, asks) = watch::channel(Vec::new());
        let task = tokio::spawn(keep_announcing(
            messenger,
            on_call_topic(),
            rosters,
            directories,
            asks,
        ));
        Self {
            task,
            roster,
            directory,
            asked,
            attempts,
            unreachable,
        }
    }

    fn knowing_everyone(roster: Roster) -> Self {
        Self::start(Some(roster), Some(alice_and_bob_in_chat()))
    }

    fn becomes(&self, roster: Roster) {
        self.roster.send_replace(Some(roster));
    }

    async fn next_attempt(&mut self) -> Attempt {
        tokio::time::timeout(PATIENCE, self.attempts.recv())
            .await
            .unwrap()
            .unwrap()
    }

    async fn next_announcement(&mut self) -> String {
        let attempt = self.next_attempt().await;
        assert!(attempt.delivered, "{attempt:?}");
        attempt.markdown
    }

    async fn announces_nothing(&mut self) {
        tokio::time::sleep(A_LONG_TIME).await;
        assert_eq!(self.attempts.try_recv(), Err(TryRecvError::Empty));
    }
}

impl Drop for Announcing {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test(start_paused = true)]
async fn the_first_announcement_is_who_is_on_call_now_in_the_given_conversation() {
    let now = lichess(&[alice()], &[bob(), carol()]);
    let mut announcing = Announcing::knowing_everyone(now.clone());

    assert_eq!(
        announcing.next_attempt().await,
        Attempt {
            to: on_call_topic(),
            markdown: first_announcement_of(&now),
            delivered: true,
        }
    );
    announcing.announces_nothing().await;
}

#[tokio::test(start_paused = true)]
async fn nothing_is_announced_until_people_are_matched() {
    let mut announcing = Announcing::start(Some(lichess(&[alice()], &[])), None);
    announcing.announces_nothing().await;

    announcing
        .directory
        .send_replace(Some(alice_and_bob_in_chat()));

    assert_eq!(
        announcing.next_announcement().await,
        format!(
            "\
:pager: **On call now:** @_**|12** for 4 hours

```spoiler schedule
**Lichess · Level 1**
- @_**|12** until {}

**Lichess · Level 2**
- nobody
```",
            this_evening()
        )
    );
}

#[tokio::test(start_paused = true)]
async fn nothing_is_announced_until_the_roster_is_read() {
    let mut announcing = Announcing::start(None, Some(alice_and_bob_in_chat()));
    announcing.announces_nothing().await;

    announcing.becomes(lichess(&[], &[carol()]));

    assert_eq!(
        announcing.next_announcement().await,
        "\
:pager: **On call now:** Carol Example

```spoiler schedule
**Lichess · Level 1**
- nobody

**Lichess · Level 2**
- Carol Example
```"
    );
}

#[tokio::test(start_paused = true)]
async fn a_later_change_announces_only_the_levels_that_changed() {
    let mut announcing = Announcing::knowing_everyone(lichess(&[bob()], &[carol()]));
    announcing.next_announcement().await;

    announcing.becomes(lichess(&[alice()], &[carol()]));

    assert_eq!(
        announcing.next_announcement().await,
        format!(
            "\
:pager: **On call changed:** @_**|12** for 4 hours

```spoiler changes
**Lichess · Level 1**
\\- @_**|13**
\\+ @_**|12** until {}
```",
            this_evening()
        )
    );
    announcing.announces_nothing().await;
}

#[tokio::test(start_paused = true)]
async fn each_change_is_announced_against_the_roster_announced_before_it() {
    let mut announcing = Announcing::knowing_everyone(lichess(&[bob()], &[]));
    announcing.next_announcement().await;

    announcing.becomes(lichess(&[bob()], &[carol()]));
    announcing.next_announcement().await;
    announcing.becomes(lichess(&[bob()], &[alice()]));

    assert_eq!(
        announcing.next_announcement().await,
        format!(
            "\
:pager: **On call changed:** @_**|13**

```spoiler changes
**Lichess · Level 2**
\\- Carol Example
\\+ @_**|12** until {}
```",
            this_evening()
        )
    );
}

#[tokio::test(start_paused = true)]
async fn a_roster_with_the_same_people_on_call_announces_nothing() {
    let mut announcing = Announcing::knowing_everyone(lichess(&[alice()], &[bob()]));
    announcing.next_announcement().await;
    let later = Shift {
        ends: Some(*THIS_EVENING + HALF_A_DAY),
        ..alice()
    };

    announcing.becomes(lichess(std::slice::from_ref(&later), &[bob()]));
    announcing.announces_nothing().await;
    announcing.becomes(lichess(&[later], &[carol()]));

    assert_eq!(
        announcing.next_announcement().await,
        "\
:pager: **On call changed:** @_**|12** for 16 hours

```spoiler changes
**Lichess · Level 2**
\\- @_**|13**
\\+ Carol Example
```"
    );
}

#[tokio::test(start_paused = true)]
async fn a_change_is_announced_with_the_people_matched_by_then() {
    let mut announcing = Announcing::knowing_everyone(lichess(&[alice()], &[]));
    announcing.next_announcement().await;

    announcing
        .directory
        .send_replace(Some(directory(&[(&alice().user, 12), (&carol().user, 14)])));
    announcing.announces_nothing().await;
    announcing.becomes(lichess(&[carol()], &[]));

    assert_eq!(
        announcing.next_announcement().await,
        "\
:pager: **On call changed:** @_**|14**

```spoiler changes
**Lichess · Level 1**
\\- @_**|12**
\\+ @_**|14**
```"
    );
}

#[tokio::test(start_paused = true)]
async fn a_failed_delivery_is_tried_again_after_thirty_seconds_with_the_roster_as_it_is_then() {
    let first = lichess(&[alice()], &[]);
    let second = lichess(&[bob()], &[]);
    let mut announcing = Announcing::knowing_everyone(first.clone());
    announcing.unreachable.store(true, Ordering::SeqCst);

    let failed = announcing.next_attempt().await;
    let failed_at = Instant::now();
    announcing.becomes(second.clone());
    announcing.unreachable.store(false, Ordering::SeqCst);
    let retried = announcing.next_attempt().await;

    assert_eq!(
        failed,
        Attempt {
            to: on_call_topic(),
            markdown: first_announcement_of(&first),
            delivered: false,
        }
    );
    assert_eq!(
        retried,
        Attempt {
            to: on_call_topic(),
            markdown: first_announcement_of(&second),
            delivered: true,
        }
    );
    assert_eq!(failed_at.elapsed(), Duration::from_secs(30));
    announcing.announces_nothing().await;
}

#[tokio::test(start_paused = true)]
async fn a_roster_counts_as_announced_only_once_its_announcement_was_delivered() {
    let mut announcing = Announcing::knowing_everyone(lichess(&[alice()], &[]));
    announcing.next_announcement().await;

    announcing.unreachable.store(true, Ordering::SeqCst);
    announcing.becomes(lichess(&[bob()], &[]));
    let failed = announcing.next_attempt().await;
    announcing.becomes(lichess(&[carol()], &[]));
    announcing.unreachable.store(false, Ordering::SeqCst);

    assert!(!failed.delivered);
    assert_eq!(
        announcing.next_announcement().await,
        "\
:pager: **On call changed:** Carol Example

```spoiler changes
**Lichess · Level 1**
\\- @_**|12**
\\+ Carol Example
```"
    );
    announcing.announces_nothing().await;
}

#[tokio::test(start_paused = true)]
async fn a_change_undone_before_the_retry_announces_nothing() {
    let mut announcing = Announcing::knowing_everyone(lichess(&[alice()], &[]));
    announcing.next_announcement().await;

    announcing.unreachable.store(true, Ordering::SeqCst);
    announcing.becomes(lichess(&[bob()], &[]));
    announcing.next_attempt().await;
    announcing.becomes(lichess(&[alice()], &[]));

    announcing.announces_nothing().await;
}

#[tokio::test(start_paused = true)]
async fn announcing_ends_once_the_roster_is_no_longer_published() {
    let mut announcing = Announcing::knowing_everyone(lichess(&[alice()], &[]));
    announcing.next_announcement().await;

    announcing.roster = watch::channel(None).0;

    assert!(
        tokio::time::timeout(PATIENCE, &mut announcing.task)
            .await
            .is_ok()
    );
}

#[tokio::test(start_paused = true)]
async fn announcing_ends_when_people_will_never_be_matched() {
    let mut announcing = Announcing::start(Some(lichess(&[alice()], &[])), None);

    announcing.directory = watch::channel(None).0;

    assert!(
        tokio::time::timeout(PATIENCE, &mut announcing.task)
            .await
            .is_ok()
    );
    assert_eq!(
        announcing.attempts.try_recv(),
        Err(TryRecvError::Disconnected)
    );
}

#[tokio::test(start_paused = true)]
async fn a_change_names_who_asked_in_which_message_only_while_the_ask_lasts() {
    let mut announcing = Announcing::knowing_everyone(lichess(&[], &[]));
    announcing.next_announcement().await;
    let on_the_primary_rota = |shifts: &[Shift]| {
        roster(&[policy(
            "PLICHES",
            "Lichess",
            &[Level {
                schedules: vec![schedule("PPRIMAR", "Primary rota")],
                ..level(1, shifts)
            }],
        )])
    };
    let asked = |user: &Shift, message: u64, lasting: Duration| Asked {
        user: user.user.id.clone(),
        schedule: ScheduleId::try_new("PPRIMAR").unwrap(),
        in_message: MessageLink {
            channel: ChannelName::try_new("sysadmin").unwrap(),
            topic: TopicName::try_new("incident").unwrap(),
            id: MessageId(message),
        },
        until: Instant::now() + lasting,
    };
    announcing.asked.send_replace(vec![
        asked(&alice(), 41, A_LONG_TIME),
        asked(&bob(), 42, Duration::ZERO),
    ]);

    announcing.becomes(on_the_primary_rota(&[alice(), bob()]));

    assert_eq!(
        announcing.next_announcement().await,
        format!(
            "\
:pager: **On call changed:** @_**|12** for 4 hours, @_**|13**
#**sysadmin>incident@41**

```spoiler changes
**Lichess · Level 1**
\\- nobody
\\+ @_**|12** until {}
\\+ @_**|13**
```",
            this_evening()
        )
    );
}
