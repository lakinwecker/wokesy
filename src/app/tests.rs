mod fake_pagerduty;
mod fake_zulip;

use std::net::SocketAddr;

use axum::http::{Method, StatusCode};
use secrecy::SecretString;
use serde_json::json;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::net::TcpStream;
use tokio::sync::oneshot;

use super::*;
use crate::chat::{ChannelName, ChatUserId, Conversation, MessageId, TopicName};
use crate::status;
use fake_pagerduty::{
    ENQUEUE, FakePagerDuty, ON_CALL, PRIMARY_ROTA_OVERRIDES, PRIMARY_ROTA_SHIFT,
    PRIMARY_ROTA_SHIFTS,
};
use fake_zulip::{ALICE, BOB, FakeZulip, Reaction, SAM};

const PATIENCE: Duration = Duration::from_secs(5);
const LONGER_THAN_A_TEST: Duration = Duration::from_hours(1);
const A_MOMENT: Duration = Duration::from_millis(250);
const KUBERNETES_GRACE_PERIOD: Duration = Duration::from_secs(30);
const LONGEST_TELEMETRY_SHUTDOWN: Duration = Duration::from_secs(10);
const ROUTING_KEY: &str = "samplekeyhere";
const SYSADMIN: &str = "sysadmin";
const STATUS: &str = "bot-status";
const HELP_STARTS: &str = "I manage who is on call in PagerDuty.";

struct Wokesy {
    pagerduty: FakePagerDuty,
    zulip: FakeZulip,
    probes: SocketAddr,
    shutdown: oneshot::Sender<Signal>,
    running: JoinHandle<anyhow::Result<()>>,
}

fn channel(name: &str) -> ChannelName {
    ChannelName::try_new(name).unwrap()
}

fn topic(name: &str) -> Conversation {
    Conversation::Topic {
        channel: channel(SYSADMIN),
        topic: TopicName::try_new(name).unwrap(),
    }
}

fn on_call_now(pagerduty: &FakePagerDuty) -> String {
    format!(
        "\
:pager: **On call now:** @_**|{BOB}** for 4 hours

```spoiler schedule
**Lichess · Level 1**
- @_**|{BOB}** until <time:{}>

**Lichess · Level 2**
- Carol Example
```",
        pagerduty.bob_is_on_call_until()
    )
}

fn on_call_topic() -> Conversation {
    topic("on call")
}

fn incident_topic() -> Conversation {
    topic("incident")
}

fn status_topic() -> Conversation {
    Conversation::Topic {
        channel: channel(STATUS),
        topic: TopicName::try_new("wokesy status").unwrap(),
    }
}

fn settings(pagerduty: &FakePagerDuty, zulip: &FakeZulip) -> Config {
    Config {
        http: http::Config::default(),
        zulip: zulip::Config {
            site: zulip.url.clone(),
            email: zulip::BotEmail::try_new("wokesy-bot@zulip.example.com").unwrap(),
            api_key: SecretString::from("key"),
            inbox: zulip::InboxConfig {
                initial_backoff: Duration::from_millis(10),
                max_backoff: Duration::from_millis(50),
            },
        },
        pagerduty: pagerduty::Config {
            token: SecretString::from("secret"),
            base_url: pagerduty.url.clone(),
            events: Some(pagerduty::EventsConfig {
                routing_key: SecretString::from(ROUTING_KEY),
                url: pagerduty.url.clone(),
            }),
        },
        announcements: on_call::Announcements {
            channel: channel(SYSADMIN),
            topic: TopicName::try_new("on call").unwrap(),
        },
        on_call: on_call::Config {
            refresh_interval: LONGER_THAN_A_TEST,
            ..on_call::Config::default()
        },
        people: people::Config {
            refresh_interval: LONGER_THAN_A_TEST,
            ..people::Config::default()
        },
        wake: wake::Config {
            schedules: fake_pagerduty::PRIMARY_ROTA.into(),
            ..wake::Config::default()
        },
        status: Some(status::Config {
            channel: channel(STATUS),
        }),
        health: health::Config {
            interval: LONGER_THAN_A_TEST,
            ..health::Config::default()
        },
        telemetry: telemetry::Config::default(),
    }
}

impl Wokesy {
    async fn start() -> Self {
        Self::start_on(FakeZulip::start().await, LONGEST_WAITS).await
    }

    async fn start_on(zulip: FakeZulip, longest_waits: LongestWaits) -> Self {
        Self::start_between(FakePagerDuty::start().await, zulip, longest_waits).await
    }

    async fn start_between(
        pagerduty: FakePagerDuty,
        zulip: FakeZulip,
        longest_waits: LongestWaits,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let probes = listener.local_addr().unwrap();
        let (shutdown, requested) = oneshot::channel();
        let running = tokio::spawn(run_until(
            settings(&pagerduty, &zulip),
            listener,
            async move { requested.await.unwrap_or(Signal::Interrupt) },
            longest_waits,
        ));
        Self {
            pagerduty,
            zulip,
            probes,
            shutdown,
            running,
        }
    }

    async fn settled() -> Self {
        Self::settled_waiting_at_most(LONGEST_WAITS).await
    }

    async fn settled_waiting_at_most(longest_waits: LongestWaits) -> Self {
        Self::start_on(FakeZulip::start().await, longest_waits)
            .await
            .once_settled()
            .await
    }

    async fn settled_on(pagerduty: FakePagerDuty) -> Self {
        Self::start_between(pagerduty, FakeZulip::start().await, LONGEST_WAITS)
            .await
            .once_settled()
            .await
    }

    async fn once_settled(self) -> Self {
        let mut status = self.zulip.posts_in(status_topic());
        self.zulip.posts_in(on_call_topic()).next().await;
        status.next().await;
        status.next().await;
        self
    }

    async fn probe(&self, path: &str) -> StatusCode {
        reqwest::get(format!("http://{}{path}", self.probes))
            .await
            .unwrap()
            .status()
    }

    async fn shut_down(self) -> (FakePagerDuty, FakeZulip, anyhow::Result<()>) {
        self.shutdown.send(Signal::Terminate).unwrap();
        let outcome = tokio::time::timeout(PATIENCE, self.running)
            .await
            .unwrap()
            .unwrap();
        (self.pagerduty, self.zulip, outcome)
    }
}

#[tokio::test]
async fn who_is_on_call_is_announced_after_start_naming_matched_people_silently() {
    let wokesy = Wokesy::start().await;

    let announced = wokesy.zulip.posts_in(on_call_topic()).next().await;

    assert_eq!(announced, on_call_now(&wokesy.pagerduty));
}

#[tokio::test]
async fn a_mention_asking_who_is_on_call_is_answered_in_its_topic() {
    let wokesy = Wokesy::settled().await;

    wokesy
        .zulip
        .mention(ALICE, SYSADMIN, "incident", "who is on call");

    let answer = wokesy.zulip.posts_in(incident_topic()).next().await;
    assert_eq!(answer, on_call_now(&wokesy.pagerduty));
}

fn check_mark_on(message: MessageId) -> Reaction {
    Reaction {
        on: message,
        emoji_name: "check".to_owned(),
    }
}

#[tokio::test]
async fn wake_me_overrides_the_configured_schedule_gets_a_check_mark_and_the_change_is_announced() {
    let wokesy = Wokesy::settled().await;
    let mut announcements = wokesy.zulip.posts_in(on_call_topic());
    assert_eq!(announcements.next().await, on_call_now(&wokesy.pagerduty));

    let wake_me = wokesy
        .zulip
        .mention(ALICE, SYSADMIN, "incident", "wake me for 4 hours");

    let acknowledged = wokesy.zulip.first_reaction().await;
    let announced = announcements.next().await;
    wokesy.zulip.mention(ALICE, SYSADMIN, "incident", "help");
    let first_in_the_topic = wokesy.zulip.posts_in(incident_topic()).next().await;
    let asked = wokesy.pagerduty.asked();
    let created = wokesy.pagerduty.changes_asked();
    let [created] = created.as_slice() else {
        panic!("expected one change in PagerDuty, got {created:#?}");
    };
    let shift = &created.body["override"];
    let moment = |field: &str| OffsetDateTime::parse(shift[field].as_str().unwrap(), &Rfc3339);
    let until = shift["end"].as_str().unwrap();
    assert!(created.is(&Method::POST, PRIMARY_ROTA_OVERRIDES));
    assert_eq!(
        shift["user"],
        json!({ "id": fake_pagerduty::ALICE, "type": "user_reference" })
    );
    assert_eq!(
        moment("end").unwrap() - moment("start").unwrap(),
        Duration::from_hours(4)
    );
    assert_eq!(acknowledged, check_mark_on(wake_me));
    assert!(
        first_in_the_topic.starts_with(HELP_STARTS),
        "{first_in_the_topic}"
    );
    assert_eq!(wokesy.zulip.reactions(), [check_mark_on(wake_me)]);
    assert_eq!(
        announced,
        format!(
            "\
:pager: **On call changed:** @_**|{ALICE}** for 4 hours
#**sysadmin>incident@{}**

```spoiler changes
**Lichess · Level 1**
\\- @_**|{BOB}**
\\+ @_**|{ALICE}** until <time:{until}>
```",
            wake_me.0
        )
    );
    let overridden = asked.iter().position(|asked| asked == created).unwrap();
    assert!(
        asked[overridden..]
            .iter()
            .any(|asked| asked.is(&Method::GET, ON_CALL))
    );
}

#[tokio::test]
async fn on_a_shift_based_schedule_wake_me_adds_a_custom_shift_and_stop_waking_me_deletes_it() {
    let wokesy = Wokesy::settled_on(FakePagerDuty::start_with_a_shift_based_rota().await).await;
    let mut announcements = wokesy.zulip.posts_in(on_call_topic());
    let mut answers = wokesy.zulip.posts_in(incident_topic());
    assert_eq!(announcements.next().await, on_call_now(&wokesy.pagerduty));

    let wake_me = wokesy
        .zulip
        .mention(ALICE, SYSADMIN, "incident", "wake me for 4 hours");
    let woken = answers.next().await;
    let added = announcements.next().await;
    let stop_waking_me = wokesy
        .zulip
        .mention(ALICE, SYSADMIN, "incident", "stop waking me");
    let stopped = wokesy.zulip.first_reaction().await;
    let removed = announcements.next().await;
    wokesy.zulip.mention(ALICE, SYSADMIN, "incident", "help");
    let next_in_the_topic = answers.next().await;

    let changes = wokesy.pagerduty.changes_asked();
    let [refused, created, deleted] = changes.as_slice() else {
        panic!("expected three changes asked of PagerDuty, got {changes:#?}");
    };
    let shift = &created.body["custom_shifts"][0];
    let moment = |field: &str| OffsetDateTime::parse(shift[field].as_str().unwrap(), &Rfc3339);
    let until = shift["end_time"].as_str().unwrap();
    let bob_until = wokesy.pagerduty.bob_is_on_call_until();
    assert!(refused.is(&Method::POST, PRIMARY_ROTA_OVERRIDES));
    assert!(created.is(&Method::POST, PRIMARY_ROTA_SHIFTS));
    assert!(deleted.is(&Method::DELETE, PRIMARY_ROTA_SHIFT));
    assert_eq!(
        shift["assignments"],
        json!([{
            "type": "shift_assignment",
            "member": { "type": "user_member", "user_id": fake_pagerduty::ALICE },
        }])
    );
    assert_eq!(
        moment("end_time").unwrap() - moment("start_time").unwrap(),
        Duration::from_hours(4)
    );
    assert_eq!(
        woken,
        format!(
            ":alarm_clock: @_**|{ALICE}** is now on call for 4 hours, together with 1 other at level 1. Level 2 (1 person) is backup."
        )
    );
    assert_eq!(
        added,
        format!(
            "\
:pager: **On call changed:** @_**|{ALICE}** for 4 hours, @_**|{BOB}** for 4 hours
#**sysadmin>incident@{}**

```spoiler changes
**Lichess · Level 1**
\\+ @_**|{ALICE}** until <time:{until}>
@_**|{BOB}** until <time:{bob_until}>
```",
            wake_me.0
        )
    );
    assert_eq!(stopped, check_mark_on(stop_waking_me));
    assert!(
        next_in_the_topic.starts_with(HELP_STARTS),
        "{next_in_the_topic}"
    );
    assert_eq!(wokesy.zulip.reactions(), [check_mark_on(stop_waking_me)]);
    assert_eq!(
        removed,
        format!(
            "\
:pager: **On call changed:** @_**|{BOB}** for 4 hours

```spoiler changes
**Lichess · Level 1**
\\- @_**|{ALICE}**
@_**|{BOB}** until <time:{bob_until}>
```"
        )
    );
}

#[tokio::test]
async fn when_zulip_refuses_the_check_mark_wake_me_is_answered_with_a_message_instead() {
    let wokesy = Wokesy::settled().await;
    let mut announcements = wokesy.zulip.posts_in(on_call_topic());
    assert_eq!(announcements.next().await, on_call_now(&wokesy.pagerduty));
    wokesy.zulip.refuses_reactions();

    wokesy
        .zulip
        .mention(ALICE, SYSADMIN, "incident", "wake me for 4 hours");

    let answer = wokesy.zulip.posts_in(incident_topic()).next().await;
    let announced = announcements.next().await;
    assert_eq!(
        answer,
        format!(
            ":alarm_clock: @_**|{ALICE}** is now on call for 4 hours. Level 2 (1 person) is backup."
        )
    );
    assert!(
        announced.starts_with(&format!(
            ":pager: **On call changed:** @_**|{ALICE}** for 4 hours"
        )),
        "{announced}"
    );
    assert_eq!(wokesy.zulip.reactions(), []);
}

#[tokio::test]
async fn a_direct_message_is_pointed_to_the_open_channels_and_changes_nothing_in_pagerduty() {
    let wokesy = Wokesy::settled().await;

    wokesy.zulip.direct_message(ALICE, "wake me for 4 hours");

    let directly = Conversation::Direct {
        user_id: ChatUserId(ALICE),
    };
    let answer = wokesy.zulip.posts_in(directly).next().await;
    assert_eq!(
        answer,
        "I only take commands in the open, in #**sysadmin**. Mention me there."
    );
    assert_eq!(wokesy.pagerduty.changes_asked(), []);
}

#[tokio::test]
async fn a_page_alerts_through_the_events_api_and_is_quoted_in_the_on_call_topic() {
    let wokesy = Wokesy::settled().await;
    let mut announcements = wokesy.zulip.posts_in(on_call_topic());
    assert_eq!(announcements.next().await, on_call_now(&wokesy.pagerduty));

    wokesy
        .zulip
        .mention(SAM, SYSADMIN, "incident", "page db is down");

    let answer = wokesy.zulip.posts_in(incident_topic()).next().await;
    let announced = announcements.next().await;
    let changes = wokesy.pagerduty.changes_asked();
    let [alert] = changes.as_slice() else {
        panic!("expected one alert in PagerDuty, got {changes:#?}");
    };
    assert!(alert.is(&Method::POST, ENQUEUE));
    assert_eq!(
        alert.body,
        json!({
            "routing_key": ROUTING_KEY,
            "event_action": "trigger",
            "payload": {
                "summary": "Sam Stranger: db is down",
                "source": "Zulip",
                "severity": "critical",
                "custom_details": { "paged_by": "Sam Stranger" },
            },
        })
    );
    assert_eq!(
        answer,
        format!(":rotating_light: Paged @_**|{BOB}**. Level 2 (1 person) is backup.")
    );
    assert_eq!(
        announced,
        format!(":rotating_light: @_**|{SAM}** paged on call:\n> db is down")
    );
}

#[tokio::test]
async fn every_request_to_pagerduty_and_zulip_says_it_comes_from_wokesy() {
    let wokesy = Wokesy::settled().await;
    wokesy
        .zulip
        .mention(SAM, SYSADMIN, "incident", "page db is down");
    wokesy.zulip.posts_in(incident_topic()).next().await;

    let mut user_agents = wokesy.zulip.user_agents();
    user_agents.extend(
        wokesy
            .pagerduty
            .asked()
            .into_iter()
            .map(|asked| asked.user_agent),
    );
    user_agents.dedup();

    assert_eq!(user_agents, [USER_AGENT]);
    assert_eq!(
        USER_AGENT,
        concat!(
            "wokesy/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/lakinwecker/wokesy)"
        )
    );
}

#[tokio::test]
async fn readyz_answers_200_once_pagerduty_was_found_available() {
    let wokesy = Wokesy::start().await;
    let mut status = wokesy.zulip.posts_in(status_topic());

    assert!(status.next().await.starts_with(concat!(
        ":rocket: wokesy v",
        env!("CARGO_PKG_VERSION"),
        " started as "
    )));
    assert_eq!(status.next().await, ":check: PagerDuty works");
    assert_eq!(wokesy.probe("/readyz").await, StatusCode::OK);
    assert_eq!(wokesy.probe("/livez").await, StatusCode::OK);
}

#[tokio::test]
async fn a_requested_shutdown_is_posted_and_ends_without_an_error() {
    let wokesy = Wokesy::settled().await;

    let (_pagerduty, zulip, outcome) = wokesy.shut_down().await;

    let last = zulip.posted().pop().unwrap();
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(last.to, status_topic());
    assert!(
        last.markdown
            .starts_with(":octagonal_sign: Shutting down (SIGTERM) after "),
        "{last:?}"
    );
}

#[tokio::test]
async fn shutdown_stops_reading_first_and_finishes_the_command_in_hand_before_it_says_goodbye() {
    let wokesy = Wokesy::settled().await;
    wokesy.pagerduty.holds_pages_back();
    wokesy
        .zulip
        .mention(SAM, SYSADMIN, "incident", "page db is down");
    wokesy.pagerduty.was_asked(Method::POST, ENQUEUE).await;
    wokesy.zulip.waits_for_messages().await;

    let Wokesy {
        pagerduty,
        zulip,
        shutdown,
        running,
        ..
    } = wokesy;
    shutdown.send(Signal::Terminate).unwrap();
    zulip.is_no_longer_read().await;
    pagerduty.lets_pages_through();
    let outcome = tokio::time::timeout(PATIENCE, running)
        .await
        .unwrap()
        .unwrap();

    let posted: Vec<String> = zulip
        .posted()
        .into_iter()
        .map(|posted| posted.markdown)
        .collect();
    let [.., answer, announced, goodbye] = posted.as_slice() else {
        panic!("expected an answer, an announcement, and a goodbye, got {posted:#?}");
    };
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(
        answer,
        &format!(":rotating_light: Paged @_**|{BOB}**. Level 2 (1 person) is backup.")
    );
    assert!(
        announced.starts_with(&format!(":rotating_light: @_**|{SAM}** paged on call:")),
        "{announced}"
    );
    assert!(
        goodbye.starts_with(":octagonal_sign: Shutting down (SIGTERM) after "),
        "{goodbye}"
    );
}

#[tokio::test]
async fn every_job_starts_while_zulip_does_not_answer_the_post_about_the_start() {
    let zulip = FakeZulip::start().await;
    zulip.never_answers_posts_in(STATUS);

    let wokesy = Wokesy::start_on(zulip, LONGEST_WAITS).await;

    let start = wokesy.zulip.was_sent_a_post_it_never_answered().await;
    let announced = wokesy.zulip.posts_in(on_call_topic()).next().await;
    wokesy
        .zulip
        .mention(ALICE, SYSADMIN, "incident", "who is on call");
    let answer = wokesy.zulip.posts_in(incident_topic()).next().await;
    assert!(start.markdown.starts_with(":rocket: wokesy v"), "{start:?}");
    assert_eq!(announced, on_call_now(&wokesy.pagerduty));
    assert_eq!(answer, on_call_now(&wokesy.pagerduty));
    assert_eq!(wokesy.probe("/livez").await, StatusCode::OK);
}

#[tokio::test]
async fn the_post_about_the_start_is_given_up_after_its_longest_wait_and_the_status_is_followed() {
    let zulip = FakeZulip::start().await;
    zulip.never_answers_posts_in(STATUS);
    let longest_waits = LongestWaits {
        status_post: A_MOMENT,
        ..LONGEST_WAITS
    };

    let wokesy = Wokesy::start_on(zulip, longest_waits).await;

    let mut unanswered = wokesy.zulip.posts_it_never_answered();
    assert!(unanswered.next().await.starts_with(":rocket: wokesy v"));
    assert_eq!(unanswered.next().await, ":check: PagerDuty works");
}

#[tokio::test]
async fn a_shutdown_post_zulip_does_not_answer_is_given_up_after_its_longest_wait() {
    let longest_waits = LongestWaits {
        status_post: A_MOMENT,
        ..LONGEST_WAITS
    };
    let wokesy = Wokesy::settled_waiting_at_most(longest_waits).await;
    wokesy.zulip.never_answers_posts_in(STATUS);

    let (_pagerduty, zulip, outcome) = wokesy.shut_down().await;

    let goodbye = zulip.was_sent_a_post_it_never_answered().await;
    assert!(outcome.is_ok(), "{outcome:?}");
    assert!(
        goodbye
            .markdown
            .starts_with(":octagonal_sign: Shutting down (SIGTERM) after "),
        "{goodbye:?}"
    );
}

#[tokio::test]
async fn a_probe_connection_left_open_mid_request_is_given_up_after_the_longest_wait_for_the_http_server()
 {
    let longest_waits = LongestWaits {
        http_server: A_MOMENT,
        ..LONGEST_WAITS
    };
    let wokesy = Wokesy::settled_waiting_at_most(longest_waits).await;
    let mid_request = TcpStream::connect(wokesy.probes).await.unwrap();
    mid_request.writable().await.unwrap();
    mid_request.try_write(b"GET /livez HTTP/1.1\r\n").unwrap();
    assert_eq!(wokesy.probe("/livez").await, StatusCode::OK);

    let (_pagerduty, zulip, outcome) = wokesy.shut_down().await;

    let last = zulip.posted().pop().unwrap();
    assert!(outcome.is_ok(), "{outcome:?}");
    assert!(
        last.markdown.starts_with(":octagonal_sign: Shutting down"),
        "{last:?}"
    );
    drop(mid_request);
}

#[test]
fn the_longest_waits_of_a_shutdown_and_of_telemetry_fit_in_the_grace_period_kubernetes_gives() {
    let longest_shutdown = LONGEST_WAITS.command_in_hand
        + LONGEST_WAITS.status_post
        + LONGEST_WAITS.http_server
        + LONGEST_TELEMETRY_SHUTDOWN;

    assert_eq!(longest_shutdown, Duration::from_secs(27));
    assert!(longest_shutdown < KUBERNETES_GRACE_PERIOD);
}

#[tokio::test]
async fn a_job_that_ends_by_itself_is_an_error_that_names_it() {
    let exit = tokio::spawn(async {}).await;

    let error = ended(ON_CALL_SYNC, exit);

    assert_eq!(format!("{error:#}"), "on-call sync stopped unexpectedly");
}

#[tokio::test]
async fn a_job_that_panics_is_an_error_that_names_it_and_carries_the_panic() {
    let exit = tokio::spawn(async { panic!("the roster is on fire") }).await;

    let error = format!("{:#}", ended(ON_CALL_SYNC, exit));

    assert!(error.starts_with("on-call sync failed: "), "{error}");
    assert!(error.contains("the roster is on fire"), "{error}");
}
