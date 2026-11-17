use tokio::time::Instant;

use crate::chat::MessageLink;
use crate::pagerduty::{ScheduleId, UserId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asked {
    pub user: UserId,
    pub schedule: ScheduleId,
    pub in_message: MessageLink,
    pub until: Instant,
}
