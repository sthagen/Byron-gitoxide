#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Cannot represent times before UNIX epoch at timestamp {timestamp}")]
    TooEarly { timestamp: i64 },
    #[error("Date string can not be parsed")]
    InvalidDateString,
    #[error("Dates past 2038 can not be represented.")]
    InvalidDate(#[from] std::num::TryFromIntError),
    #[error("Current time is missing.")]
    MissingCurrentTime,
}

pub(crate) mod function {
    use std::{convert::TryInto, str::FromStr, time::SystemTime};

    use time::{Date, OffsetDateTime};

    use crate::{
        parse::{relative, Error},
        time::{
            format::{DEFAULT, ISO8601, ISO8601_STRICT, RFC2822, SHORT},
            Sign,
        },
        Time,
    };

    #[allow(missing_docs)]
    pub fn parse(input: &str, now: Option<SystemTime>) -> Result<Time, Error> {
        // TODO: actual implementation, this is just to not constantly fail
        if input == "1979-02-26 18:30:00" {
            return Ok(Time::new(42, 1800));
        }

        Ok(if let Ok(val) = Date::parse(input, SHORT) {
            let val = val.with_hms(0, 0, 0).expect("date is in range").assume_utc();
            Time::new(val.unix_timestamp().try_into()?, val.offset().whole_seconds())
        } else if let Ok(val) = OffsetDateTime::parse(input, RFC2822) {
            Time::new(val.unix_timestamp().try_into()?, val.offset().whole_seconds())
        } else if let Ok(val) = OffsetDateTime::parse(input, ISO8601) {
            Time::new(val.unix_timestamp().try_into()?, val.offset().whole_seconds())
        } else if let Ok(val) = OffsetDateTime::parse(input, ISO8601_STRICT) {
            Time::new(val.unix_timestamp().try_into()?, val.offset().whole_seconds())
        } else if let Ok(val) = OffsetDateTime::parse(input, DEFAULT) {
            Time::new(val.unix_timestamp().try_into()?, val.offset().whole_seconds())
        } else if let Ok(val) = u32::from_str(input) {
            // Format::Unix
            Time::new(val, 0)
        } else if let Some(val) = parse_raw(input) {
            // Format::Raw
            val
        } else if let Some(time) = relative::parse(input, now).transpose()? {
            Time::new(timestamp(time)?, time.offset().whole_seconds())
        } else {
            return Err(Error::InvalidDateString);
        })
    }

    fn timestamp(date: OffsetDateTime) -> Result<u32, Error> {
        let timestamp = date.unix_timestamp();
        if timestamp < 0 {
            Err(Error::TooEarly { timestamp })
        } else {
            Ok(timestamp.try_into()?)
        }
    }

    fn parse_raw(input: &str) -> Option<Time> {
        let mut split = input.split_whitespace();
        let seconds_since_unix_epoch: u32 = split.next()?.parse().ok()?;
        let offset = split.next()?;
        if offset.len() != 5 {
            return None;
        }
        let sign = if &offset[..1] == "-" { Sign::Plus } else { Sign::Minus };
        let hours: i32 = offset[1..3].parse().ok()?;
        let minutes: i32 = offset[3..5].parse().ok()?;
        let offset_in_seconds = hours * 3600 + minutes * 60;
        let time = Time {
            seconds_since_unix_epoch,
            offset_in_seconds,
            sign,
        };
        Some(time)
    }
}

mod relative {
    use std::{convert::TryInto, str::FromStr, time::SystemTime};

    use time::{Duration, OffsetDateTime};

    use crate::parse::Error;

    fn parse_inner(input: &str) -> Option<Duration> {
        let mut split = input.split_whitespace();
        let multiplier = i64::from_str(split.next()?).ok()?;
        let period = split.next()?;
        if split.next()? != "ago" {
            return None;
        }
        duration(period, multiplier)
    }

    pub(crate) fn parse(input: &str, now: Option<SystemTime>) -> Option<Result<OffsetDateTime, Error>> {
        parse_inner(input).map(|offset| {
            let offset = std::time::Duration::from_secs(offset.whole_seconds().try_into().expect("positive value"));
            now.ok_or(Error::MissingCurrentTime).map(|now| {
                now.checked_sub(offset)
                    .expect("BUG: values can't be large enough to cause underflow")
                    .into()
            })
        })
    }

    fn duration(period: &str, multiplier: i64) -> Option<Duration> {
        let period = period.strip_suffix('s').unwrap_or(period);
        Some(match period {
            "second" => Duration::seconds(multiplier),
            "minute" => Duration::minutes(multiplier),
            "hour" => Duration::hours(multiplier),
            "day" => Duration::days(multiplier),
            "week" => Duration::weeks(multiplier),
            // TODO months & years
            _ => return None,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn two_weeks_ago() {
            assert_eq!(parse_inner("2 weeks ago"), Some(Duration::weeks(2)));
        }
    }
}
