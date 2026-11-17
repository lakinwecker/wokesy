use super::*;

const A_MINUTE: u64 = 60;
const AN_HOUR: u64 = 60 * A_MINUTE;
const A_DAY: u64 = 24 * AN_HOUR;

#[test]
fn time_left_is_told_in_the_largest_unit_that_fits_rounded_down() {
    for (seconds, told) in [
        (A_MINUTE, "2 minutes"),
        (2 * A_MINUTE, "3 minutes"),
        (45 * A_MINUTE, "46 minutes"),
        (58 * A_MINUTE, "59 minutes"),
        (AN_HOUR, "1 hour"),
        (2 * AN_HOUR - 2 * A_MINUTE, "1 hour"),
        (2 * AN_HOUR, "2 hours"),
        (18 * AN_HOUR, "18 hours"),
        (A_DAY - 2 * A_MINUTE, "23 hours"),
        (A_DAY, "1 day"),
        (2 * A_DAY - 2 * A_MINUTE, "1 day"),
        (3 * A_DAY, "3 days"),
        (7 * A_DAY - 2 * A_MINUTE, "6 days"),
        (7 * A_DAY, "1 week"),
        (14 * A_DAY, "2 weeks"),
        (30 * A_DAY - 2 * A_MINUTE, "4 weeks"),
        (30 * A_DAY, "1 month"),
        (90 * A_DAY, "3 months"),
        (365 * A_DAY - 2 * A_MINUTE, "12 months"),
        (365 * A_DAY, "1 year"),
        (730 * A_DAY - 2 * A_MINUTE, "1 year"),
        (730 * A_DAY, "2 years"),
    ] {
        assert_eq!(in_one_unit(Duration::from_secs(seconds)), told, "{seconds}");
    }
}

#[test]
fn time_left_is_counted_as_if_the_message_was_written_up_to_90_seconds_late() {
    for (seconds, told) in [
        (4 * AN_HOUR - 1, "4 hours"),
        (4 * AN_HOUR - 90, "4 hours"),
        (4 * AN_HOUR - 91, "3 hours"),
        (58 * A_MINUTE + 29, "59 minutes"),
        (58 * A_MINUTE + 30, "1 hour"),
        (7 * A_DAY - 1, "1 week"),
    ] {
        assert_eq!(in_one_unit(Duration::from_secs(seconds)), told, "{seconds}");
    }
}

#[test]
fn time_left_is_never_told_as_less_than_a_minute() {
    for seconds in [0, 1, 29] {
        assert_eq!(
            in_one_unit(Duration::from_secs(seconds)),
            "1 minute",
            "{seconds}"
        );
    }
}

#[test]
fn a_length_of_time_is_told_in_full_in_days_hours_and_minutes() {
    for (seconds, told) in [
        (A_MINUTE, "1 minute"),
        (30 * A_MINUTE, "30 minutes"),
        (AN_HOUR, "1 hour"),
        (4 * AN_HOUR, "4 hours"),
        (90 * A_MINUTE, "1 hour 30 minutes"),
        (A_DAY, "1 day"),
        (2 * A_DAY, "2 days"),
        (7 * A_DAY, "7 days"),
        (
            A_DAY + 2 * AN_HOUR + 5 * A_MINUTE,
            "1 day 2 hours 5 minutes",
        ),
        (3 * A_DAY + A_MINUTE, "3 days 1 minute"),
    ] {
        assert_eq!(in_words(Duration::from_secs(seconds)), told, "{seconds}");
    }
}
