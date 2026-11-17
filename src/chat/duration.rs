use std::time::Duration;

#[derive(Clone, Copy)]
struct Unit {
    seconds: u64,
    one: &'static str,
    many: &'static str,
}

const MINUTE: Unit = Unit {
    seconds: 60,
    one: "minute",
    many: "minutes",
};
const HOUR: Unit = Unit {
    seconds: 60 * MINUTE.seconds,
    one: "hour",
    many: "hours",
};
const DAY: Unit = Unit {
    seconds: 24 * HOUR.seconds,
    one: "day",
    many: "days",
};
const WEEK: Unit = Unit {
    seconds: 7 * DAY.seconds,
    one: "week",
    many: "weeks",
};
const MONTH: Unit = Unit {
    seconds: 30 * DAY.seconds,
    one: "month",
    many: "months",
};
const YEAR: Unit = Unit {
    seconds: 365 * DAY.seconds,
    one: "year",
    many: "years",
};
const EVERY_UNIT: [Unit; 6] = [YEAR, MONTH, WEEK, DAY, HOUR, MINUTE];
const EXACT_UNITS: [Unit; 3] = [DAY, HOUR, MINUTE];
const WRITTEN_LATE_BY: u64 = 90;
const BETWEEN_UNITS: &str = " ";

impl Unit {
    fn counted(self, amount: u64) -> String {
        if amount == 1 {
            format!("1 {}", self.one)
        } else {
            format!("{amount} {}", self.many)
        }
    }
}

/// ```
/// use std::time::Duration;
///
/// use wokesy::chat::in_words;
///
/// assert_eq!(in_words(Duration::from_secs(30 * 60)), "30 minutes");
/// assert_eq!(in_words(Duration::from_secs(4 * 60 * 60)), "4 hours");
/// assert_eq!(in_words(Duration::from_secs(90 * 60)), "1 hour 30 minutes");
/// assert_eq!(in_words(Duration::from_secs(14 * 24 * 60 * 60)), "14 days");
/// assert_eq!(in_words(Duration::from_secs(59)), "1 minute");
/// ```
#[must_use]
pub fn in_words(lasting: Duration) -> String {
    let mut seconds = lasting.as_secs().max(MINUTE.seconds);
    let mut units = Vec::new();
    for unit in EXACT_UNITS {
        let amount = seconds / unit.seconds;
        seconds %= unit.seconds;
        if amount > 0 {
            units.push(unit.counted(amount));
        }
    }
    units.join(BETWEEN_UNITS)
}

/// ```
/// use std::time::Duration;
///
/// use wokesy::chat::in_one_unit;
///
/// let hours = |hours: u64| Duration::from_secs(hours * 60 * 60);
///
/// assert_eq!(in_one_unit(Duration::from_secs(44 * 60)), "45 minutes");
/// assert_eq!(in_one_unit(hours(3) - Duration::from_secs(90)), "3 hours");
/// assert_eq!(in_one_unit(hours(3) - Duration::from_secs(91)), "2 hours");
/// assert_eq!(in_one_unit(hours(4) + Duration::from_secs(58 * 60)), "4 hours");
/// assert_eq!(in_one_unit(hours(3 * 24)), "3 days");
/// assert_eq!(in_one_unit(hours(15 * 24)), "2 weeks");
/// assert_eq!(in_one_unit(hours(100 * 24)), "3 months");
/// assert_eq!(in_one_unit(hours(400 * 24)), "1 year");
/// assert_eq!(in_one_unit(Duration::ZERO), "1 minute");
/// ```
#[must_use]
pub fn in_one_unit(lasting: Duration) -> String {
    let seconds = lasting.as_secs().saturating_add(WRITTEN_LATE_BY);
    EVERY_UNIT
        .into_iter()
        .find(|unit| seconds >= unit.seconds)
        .map_or_else(
            || MINUTE.counted(1),
            |unit| unit.counted(seconds / unit.seconds),
        )
}

#[cfg(test)]
mod tests;
