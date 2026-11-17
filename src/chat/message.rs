use serde::Deserialize;

use super::conversation::{ChannelName, TopicName};

const ESCAPED: [(&str, &str); 7] = [
    ("&", "&amp;"),
    ("`", "&#96;"),
    (">", "&gt;"),
    ("*", "&#42;"),
    ("[", "&#91;"),
    ("]", "&#93;"),
    ("$$", "&#36;&#36;"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub struct MessageId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageLink {
    pub channel: ChannelName,
    pub topic: TopicName,
    pub id: MessageId,
}

impl MessageLink {
    /// ```
    /// use wokesy::chat::{ChannelName, MessageId, MessageLink, TopicName};
    ///
    /// let link = |channel: &str, topic: &str| MessageLink {
    ///     channel: ChannelName::try_new(channel).unwrap(),
    ///     topic: TopicName::try_new(topic).unwrap(),
    ///     id: MessageId(42),
    /// };
    ///
    /// assert_eq!(link("sysadmin", "incident").in_chat(), "#**sysadmin>incident@42**");
    /// assert_eq!(
    ///     link("ops*", "a > b & `c` @**all**").in_chat(),
    ///     "#**ops&#42;>a &gt; b &amp; &#96;c&#96; @&#42;&#42;all&#42;&#42;@42**"
    /// );
    /// ```
    #[must_use]
    pub fn in_chat(&self) -> String {
        format!(
            "#**{}>{}@{}**",
            escaped(&self.channel),
            escaped(&self.topic),
            self.id.0
        )
    }
}

fn escaped(name: &str) -> String {
    ESCAPED
        .iter()
        .fold(name.to_owned(), |name, (from, to)| name.replace(from, to))
}
