mod backup;
mod own_changes;
mod paging;
mod waking;

use std::collections::BTreeSet;
use std::sync::Arc;

use time::OffsetDateTime;
use tokio::sync::{Notify, watch};

use super::config::Config;
use super::parsing::{Command, parse};
use crate::chat::{ChannelName, Conversation, IncomingMessage, OutgoingMessage};
use crate::on_call::{Asked, Roster, on_call_now};
use crate::pagerduty;
use crate::people::Directory;

use own_changes::OwnChange;
use paging::Paged;

const WAKING_IS_OFF: &str = "Waking is not set up here. Change the schedule in PagerDuty directly.";
const STILL_READING: &str = "I'm still reading PagerDuty. Try again in a minute.";
const UNKNOWN_SENDER: &str = "I don't know which PagerDuty user you are. I match people by email address, then by full name. Ask whoever runs wokesy to link your accounts.";
const UNRECOGNIZED: &str = "I don't know that command. Send `help` to see the commands I know.";
const HELP: &str = "\
I manage who is on call in PagerDuty.
- `page lichess.org is down` alerts whoever is on call, through PagerDuty. Anyone here can use it. After a page I wait a while before I page again.
- `wake me for 4 hours` puts you on the wake schedule for that long. PagerDuty ends it by itself when the time is up.
- `stop waking me` ends that early.
- `back to defaults` ends every override and shift I made, for you or anyone else, so PagerDuty's own schedules decide again.
- `who is on call` shows who is on call now.";
const BETWEEN_OPEN_CHANNELS: &str = " or ";
const FIRST_LINE: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    Message(OutgoingMessage),
    Acknowledgement { otherwise: OutgoingMessage },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    pub reply: Reply,
    pub announcement: Option<OutgoingMessage>,
}

#[derive(Debug)]
pub struct Commands {
    pagerduty: pagerduty::Api,
    events: Option<pagerduty::Events>,
    roster: watch::Receiver<Option<Roster>>,
    directory: watch::Receiver<Option<Directory>>,
    refresh: Arc<Notify>,
    config: Config,
    open_channels: Vec<ChannelName>,
    last_page: Option<Paged>,
    own_changes: Vec<OwnChange>,
    shift_based: BTreeSet<pagerduty::ScheduleId>,
    asked: watch::Sender<Vec<Asked>>,
}

impl Reply {
    fn message(markdown: impl Into<String>) -> Self {
        Self::Message(OutgoingMessage::new(markdown.into()))
    }

    fn acknowledgement(otherwise: impl Into<String>) -> Self {
        Self::Acknowledgement {
            otherwise: OutgoingMessage::new(otherwise.into()),
        }
    }
}

impl Answer {
    fn only(reply: impl Into<String>) -> Self {
        Self {
            reply: Reply::message(reply),
            announcement: None,
        }
    }
}

impl Commands {
    #[must_use]
    pub fn new(
        pagerduty: pagerduty::Api,
        events: Option<pagerduty::Events>,
        roster: watch::Receiver<Option<Roster>>,
        directory: watch::Receiver<Option<Directory>>,
        refresh: Arc<Notify>,
        config: Config,
        open_channels: Vec<ChannelName>,
    ) -> Self {
        Self {
            pagerduty,
            events,
            roster,
            directory,
            refresh,
            config,
            open_channels,
            last_page: None,
            own_changes: Vec::new(),
            shift_based: BTreeSet::new(),
            asked: watch::Sender::new(Vec::new()),
        }
    }

    #[must_use]
    pub fn asked(&self) -> watch::Receiver<Vec<Asked>> {
        self.asked.subscribe()
    }

    /// ```
    /// use std::sync::Arc;
    ///
    /// use tokio::sync::{Notify, watch};
    /// use wokesy::chat::{
    ///     ChannelName, ChatUserId, CommandText, Conversation, IncomingMessage, MessageId,
    ///     OutgoingMessage,
    /// };
    /// use wokesy::pagerduty;
    /// use wokesy::people::Name;
    /// use wokesy::wake::{Commands, Config, Reply};
    ///
    /// let pagerduty = pagerduty::Config {
    ///     token: "secret".into(),
    ///     base_url: "https://api.pagerduty.com/".parse().unwrap(),
    ///     events: None,
    /// };
    /// let (_roster, roster) = watch::channel(None);
    /// let (_directory, directory) = watch::channel(None);
    /// let config = Config::default();
    /// let open_channels = config.open_channels(&ChannelName::try_new("sysadmin").unwrap());
    /// let mut commands = Commands::new(
    ///     pagerduty::Api::new(reqwest::Client::new(), &pagerduty),
    ///     None,
    ///     roster,
    ///     directory,
    ///     Arc::new(Notify::new()),
    ///     config,
    ///     open_channels,
    /// );
    /// let sender = ChatUserId(12);
    /// let in_private = IncomingMessage {
    ///     id: MessageId(1),
    ///     sender,
    ///     sender_name: Name::try_new("Alice Example").unwrap(),
    ///     reply_to: Conversation::Direct { user_id: sender },
    ///     text: CommandText::new("who is on call"),
    /// };
    ///
    /// let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    /// let answer = runtime.block_on(commands.answer(&in_private));
    ///
    /// let pointer = "I only take commands in the open, in #**sysadmin**. Mention me there.";
    /// assert_eq!(
    ///     answer.reply,
    ///     Reply::Message(OutgoingMessage::new(pointer.to_owned()))
    /// );
    /// assert_eq!(answer.announcement, None);
    /// ```
    pub async fn answer(&mut self, message: &IncomingMessage) -> Answer {
        if !self.takes_commands_in(&message.reply_to) {
            return Answer::only(self.only_in_the_open());
        }
        let reply = match (parse(&message.text), self.read()) {
            (Command::Page(trouble), read) => {
                return self.page(message, trouble, read.as_ref()).await;
            }
            (Command::Help, _) => Reply::message(HELP),
            (Command::Unrecognized, _) => Reply::message(UNRECOGNIZED),
            (Command::Wake(_), _) if self.config.schedules.ids().is_empty() => {
                Reply::message(WAKING_IS_OFF)
            }
            (Command::Wake(_) | Command::Stop | Command::Reset | Command::Who, None) => {
                Reply::message(STILL_READING)
            }
            (Command::Wake(lasting), Some((roster, directory))) => {
                self.wake(message, lasting, &roster, &directory).await
            }
            (Command::Stop, Some((_, directory))) => self.stop(message.sender, &directory).await,
            (Command::Reset, Some((_, directory))) => self.reset(message.sender, &directory).await,
            (Command::Who, Some((roster, directory))) => {
                Reply::message(on_call_now(&roster, &directory, OffsetDateTime::now_utc()))
            }
        };
        Answer {
            reply,
            announcement: None,
        }
    }

    fn takes_commands_in(&self, conversation: &Conversation) -> bool {
        match conversation {
            Conversation::Direct { .. } => false,
            Conversation::Topic { channel, .. } => self.open_channels.contains(channel),
        }
    }

    fn only_in_the_open(&self) -> String {
        let channels: Vec<String> = self
            .open_channels
            .iter()
            .map(|channel| format!("#**{}**", channel.as_str()))
            .collect();
        format!(
            "I only take commands in the open, in {}. Mention me there.",
            channels.join(BETWEEN_OPEN_CHANNELS)
        )
    }

    fn read(&self) -> Option<(Roster, Directory)> {
        let roster = self.roster.borrow().clone()?;
        let directory = self.directory.borrow().clone()?;
        Some((roster, directory))
    }
}

#[cfg(test)]
mod tests;
