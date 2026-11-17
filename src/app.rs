pub mod config;
mod http;
pub mod telemetry;

use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::{Notify, mpsc, watch};
use tokio::task::{JoinError, JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::health::{self, Health};
use crate::on_call::{self, Roster};
use crate::people::{self, Directory};
use crate::status::{Shutdown, Signal, Status};
use crate::zulip::{self, ZulipMembers, ZulipMessenger};
use crate::{pagerduty, wake};
use config::Config;

const USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (+",
    env!("CARGO_PKG_REPOSITORY"),
    ")"
);
const HEALTH_CHECK: &str = "health check";
const PEOPLE_MATCHING: &str = "people matching";
const ON_CALL_SYNC: &str = "on-call sync";
const ON_CALL_ANNOUNCEMENTS: &str = "on-call announcements";
const ZULIP_INBOX: &str = "zulip inbox";
const COMMANDS: &str = "commands";
const HTTP_SERVER: &str = "http server";
const START_POST: &str = "posting the start";
const SHUTDOWN_POST: &str = "posting the shutdown";
const COMMANDS_WAITING_AT_MOST: usize = 16;
const LONGEST_WAITS: LongestWaits = LongestWaits {
    command_in_hand: Duration::from_secs(10),
    status_post: Duration::from_secs(5),
    http_server: Duration::from_secs(2),
};

#[derive(Clone, Copy, Debug)]
struct LongestWaits {
    command_in_hand: Duration,
    status_post: Duration,
    http_server: Duration,
}

pub fn http_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder().user_agent(USER_AGENT).build()
}

pub async fn run(config: Config) -> anyhow::Result<()> {
    let listener = http::bind(&config.http).await?;
    run_until(config, listener, shutdown_requested(), LONGEST_WAITS).await
}

async fn run_until(
    config: Config,
    listener: TcpListener,
    shutdown: impl Future<Output = Signal>,
    longest_waits: LongestWaits,
) -> anyhow::Result<()> {
    let http = http_client()?;
    let messenger = ZulipMessenger::new(http.clone(), &config.zulip);
    let mut status = Status::new(messenger.clone(), config.status.clone());
    let (health, checked) = watch::channel(None);

    let mut probes = Probes::serve(listener, checked.clone());
    let mut jobs = Jobs::start(&config, &http, messenger, health);

    let down: anyhow::Result<Signal> = tokio::select! {
        biased;
        signal = shutdown => Ok(signal),
        error = jobs.stopped() => Err(error),
        error = probes.stopped() => Err(error),
        () = report(&mut status, checked, longest_waits.status_post) => {
            Err(stopped_unexpectedly(HEALTH_CHECK))
        }
    };

    jobs.stop(longest_waits.command_in_hand).await;
    let stopping = status.stopping(shutdown_after(&down));
    within(longest_waits.status_post, SHUTDOWN_POST, stopping).await;
    exit_with(down, probes.stop(longest_waits.http_server).await)
}

async fn report(
    status: &mut Status<ZulipMessenger>,
    health: watch::Receiver<Option<Health>>,
    longest_wait_for_the_start_post: Duration,
) {
    within(
        longest_wait_for_the_start_post,
        START_POST,
        status.started(),
    )
    .await;
    status.follow(health).await;
}

async fn within(longest_wait: Duration, doing: &'static str, done: impl Future<Output = ()>) {
    if tokio::time::timeout(longest_wait, done).await.is_err() {
        tracing::warn!(?longest_wait, doing, "gave up waiting");
    }
}

fn shutdown_after(down: &anyhow::Result<Signal>) -> Shutdown {
    match down {
        Ok(signal) => Shutdown::Requested(*signal),
        Err(error) => Shutdown::Failed(format!("{error:#}")),
    }
}

struct Jobs {
    health: JoinHandle<()>,
    people: JoinHandle<()>,
    sync: JoinHandle<()>,
    announcing: JoinHandle<()>,
    inbox: JoinHandle<()>,
    answering: JoinHandle<()>,
}

impl Jobs {
    fn start(
        config: &Config,
        http: &reqwest::Client,
        messenger: ZulipMessenger,
        health: watch::Sender<Option<Health>>,
    ) -> Self {
        let pagerduty = pagerduty::Api::new(http.clone(), &config.pagerduty);
        let zulip = zulip::Api::new(http.clone(), &config.zulip);
        let members = ZulipMembers::new(http.clone(), &config.zulip);
        let announcements = config.announcements.conversation();
        let (roster, rosters) = watch::channel(None);
        let (directory, directories) = watch::channel(None);
        let (incoming, received) = mpsc::channel(COMMANDS_WAITING_AT_MOST);
        let refresh = Arc::new(Notify::new());
        let commands = commands(
            config,
            http,
            pagerduty.clone(),
            rosters.clone(),
            directories.clone(),
            Arc::clone(&refresh),
        );
        let asked = commands.asked();
        Self {
            health: tokio::spawn(health::keep_checking(
                config.health,
                pagerduty.clone(),
                zulip.clone(),
                health,
            )),
            people: tokio::spawn(people::keep_matched(
                pagerduty.clone(),
                members,
                config.people.clone(),
                directory,
            )),
            sync: tokio::spawn(on_call::keep_in_sync(
                pagerduty,
                config.on_call.clone(),
                roster,
                refresh,
            )),
            announcing: tokio::spawn(on_call::keep_announcing(
                messenger.clone(),
                announcements.clone(),
                rosters,
                directories,
                asked,
            )),
            inbox: tokio::spawn(zulip::keep_reading(
                zulip,
                config.zulip.inbox.clone(),
                incoming,
            )),
            answering: tokio::spawn(wake::keep_answering(
                commands,
                messenger,
                announcements,
                received,
            )),
        }
    }

    async fn stopped(&mut self) -> anyhow::Error {
        tokio::select! {
            exit = &mut self.health => ended(HEALTH_CHECK, exit),
            exit = &mut self.people => ended(PEOPLE_MATCHING, exit),
            exit = &mut self.sync => ended(ON_CALL_SYNC, exit),
            exit = &mut self.announcing => ended(ON_CALL_ANNOUNCEMENTS, exit),
            exit = &mut self.inbox => ended(ZULIP_INBOX, exit),
            exit = &mut self.answering => ended(COMMANDS, exit),
        }
    }

    async fn stop(&mut self, longest_wait_command_in_hand: Duration) {
        self.inbox.abort();
        self.finish_the_command_in_hand(longest_wait_command_in_hand)
            .await;
        self.answering.abort();
        self.announcing.abort();
        self.sync.abort();
        self.people.abort();
        self.health.abort();
    }

    async fn finish_the_command_in_hand(&mut self, waited: Duration) {
        if self.answering.is_finished() {
            return;
        }
        match tokio::time::timeout(waited, &mut self.answering).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tracing::warn!(error = %error, "commands failed while finishing the command in hand");
            }
            Err(_) => tracing::warn!(?waited, "the command in hand did not finish in time"),
        }
    }
}

fn commands(
    config: &Config,
    http: &reqwest::Client,
    pagerduty: pagerduty::Api,
    roster: watch::Receiver<Option<Roster>>,
    directory: watch::Receiver<Option<Directory>>,
    refresh: Arc<Notify>,
) -> wake::Commands {
    let events = config
        .pagerduty
        .events
        .as_ref()
        .map(|events| pagerduty::Events::new(http.clone(), events));
    if events.is_none() {
        tracing::info!("paging is off because no events routing key is set");
    }
    wake::Commands::new(
        pagerduty,
        events,
        roster,
        directory,
        refresh,
        config.wake.clone(),
        config.wake.open_channels(&config.announcements.channel),
    )
}

struct Probes {
    serving: Option<JoinHandle<std::io::Result<()>>>,
    shutdown: CancellationToken,
}

impl Probes {
    fn serve(listener: TcpListener, health: watch::Receiver<Option<Health>>) -> Self {
        let shutdown = CancellationToken::new();
        let serving = tokio::spawn(http::serve(
            listener,
            http::router(health),
            shutdown.clone().cancelled_owned(),
        ));
        Self {
            serving: Some(serving),
            shutdown,
        }
    }

    async fn stopped(&mut self) -> anyhow::Error {
        let Some(serving) = &mut self.serving else {
            return std::future::pending().await;
        };
        let exit = serving.await;
        self.serving = None;
        match exit {
            Ok(Ok(())) => stopped_unexpectedly(HTTP_SERVER),
            Ok(Err(error)) => failed(HTTP_SERVER, error),
            Err(error) => failed(HTTP_SERVER, error),
        }
    }

    async fn stop(self, longest_wait: Duration) -> anyhow::Result<()> {
        self.shutdown.cancel();
        let Some(mut serving) = self.serving else {
            return Ok(());
        };
        if let Ok(exit) = tokio::time::timeout(longest_wait, &mut serving).await {
            return Ok(exit??);
        }
        tracing::warn!(?longest_wait, "the http server did not shut down in time");
        serving.abort();
        Ok(())
    }
}

fn ended(job: &str, exit: Result<(), JoinError>) -> anyhow::Error {
    match exit {
        Ok(()) => stopped_unexpectedly(job),
        Err(error) => failed(job, error),
    }
}

fn stopped_unexpectedly(job: &str) -> anyhow::Error {
    anyhow::anyhow!("{job} stopped unexpectedly")
}

fn failed(job: &str, error: impl std::error::Error + Send + Sync + 'static) -> anyhow::Error {
    anyhow::Error::new(error).context(format!("{job} failed"))
}

fn exit_with(down: anyhow::Result<Signal>, http_exit: anyhow::Result<()>) -> anyhow::Result<()> {
    match down {
        Ok(_) => {
            http_exit?;
            tracing::info!("shutdown complete");
            Ok(())
        }
        Err(error) => {
            if let Err(http_error) = http_exit {
                tracing::warn!(error = %http_error, "http server failed while shutting down");
            }
            Err(error)
        }
    }
}

async fn shutdown_requested() -> Signal {
    let terminate = async {
        match signal(SignalKind::terminate()) {
            Ok(mut terminations) => {
                terminations.recv().await;
            }
            Err(error) => {
                tracing::warn!(error = %error, "cannot listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    let received = tokio::select! {
        _ = tokio::signal::ctrl_c() => Signal::Interrupt,
        () = terminate => Signal::Terminate,
    };
    tracing::info!(signal = %received, "shutdown requested");
    received
}

#[cfg(test)]
mod tests;
