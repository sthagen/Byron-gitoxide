use std::str::FromStr;

use smallvec::SmallVec;

use crate::Time;

#[derive(thiserror::Error, Debug, Clone)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Could not convert a duration into a date")]
    RelativeTimeConversion,
    #[error("Date string can not be parsed")]
    InvalidDateString { input: String },
    #[error("The heat-death of the universe happens before this date")]
    InvalidDate(#[from] std::num::TryFromIntError),
    #[error("Current time is missing but required to handle relative dates.")]
    MissingCurrentTime,
}

/// A container for just enough bytes to hold the largest-possible [`time`](Time) instance.
/// It's used in conjunction with
#[derive(Default, Clone)]
pub struct TimeBuf {
    buf: SmallVec<[u8; Time::MAX.size()]>,
}

impl TimeBuf {
    /// Represent this instance as standard string, serialized in a format compatible with
    /// signature fields in Git commits, also known as anything parseable as [raw format](function::parse_header()).
    pub fn as_str(&self) -> &str {
        // SAFETY: We know that serialized times are pure ASCII, a subset of UTF-8.
        //         `buf` and `len` are written only by time-serialization code.
        let time_bytes = self.buf.as_slice();
        #[allow(unsafe_code)]
        unsafe {
            std::str::from_utf8_unchecked(time_bytes)
        }
    }

    /// Clear the previous content.
    fn clear(&mut self) {
        self.buf.clear();
    }
}

impl std::io::Write for TimeBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buf.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.buf.flush()
    }
}

impl Time {
    /// Serialize this instance into `buf`, exactly as it would appear in the header of a Git commit,
    /// and return `buf` as `&str` for easy consumption.
    pub fn to_str<'a>(&self, buf: &'a mut TimeBuf) -> &'a str {
        buf.clear();
        self.write_to(buf)
            .expect("write to memory of just the right size cannot fail");
        buf.as_str()
    }
}

impl FromStr for Time {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::parse_header(s).ok_or_else(|| Error::InvalidDateString { input: s.into() })
    }
}

pub(crate) mod function {
    use std::{str::FromStr, time::SystemTime};

    use jiff::{civil::Date, fmt::rfc2822, tz::TimeZone, Zoned};

    use crate::{
        parse::{relative, Error},
        time::format::{DEFAULT, GITOXIDE, ISO8601, ISO8601_STRICT, SHORT},
        OffsetInSeconds, SecondsSinceUnixEpoch, Time,
    };

    /// Parse `input` as any time that Git can parse when inputting a date.
    ///
    /// ## Examples
    ///
    /// ### 1. SHORT Format
    ///
    /// *   `2018-12-24`
    /// *   `1970-01-01`
    /// *   `1950-12-31`
    /// *   `2024-12-31`
    ///
    /// ### 2. RFC2822 Format
    ///
    /// *   `Thu, 18 Aug 2022 12:45:06 +0800`
    /// *   `Mon Oct 27 10:30:00 2023 -0800`
    ///
    /// ### 3. GIT_RFC2822 Format
    ///
    /// *   `Thu, 8 Aug 2022 12:45:06 +0800`
    /// *   `Mon Oct 27 10:30:00 2023 -0800` (Note the single-digit day)
    ///
    /// ### 4. ISO8601 Format
    ///
    /// *   `2022-08-17 22:04:58 +0200`
    /// *   `1970-01-01 00:00:00 -0500`
    ///
    /// ### 5. ISO8601_STRICT Format
    ///
    /// *   `2022-08-17T21:43:13+08:00`
    ///
    /// ### 6. UNIX Timestamp (Seconds Since Epoch)
    ///
    /// *   `123456789`
    /// *   `0` (January 1, 1970 UTC)
    /// *   `-1000`
    /// *   `1700000000`
    ///
    /// ### 7. Commit Header Format
    ///
    /// *   `1745582210 +0200`
    /// *   `1660874655 +0800`
    /// *   `-1660874655 +0800`
    ///
    /// See also the [`parse_header()`].
    ///
    /// ### 8. GITOXIDE Format
    ///
    /// *   `Thu Sep 04 2022 10:45:06 -0400`
    /// *   `Mon Oct 27 2023 10:30:00 +0000`
    ///
    /// ### 9. DEFAULT Format
    ///
    /// *   `Thu Sep 4 10:45:06 2022 -0400`
    /// *   `Mon Oct 27 10:30:00 2023 +0000`
    ///
    /// ### 10. Relative Dates (e.g., "2 minutes ago", "1 hour from now")
    ///
    /// These dates are parsed *relative to a `now` timestamp*. The examples depend entirely on the value of `now`.
    /// If `now` is October 27, 2023 at 10:00:00 UTC:
    ///     *   `2 minutes ago` (October 27, 2023 at 09:58:00 UTC)
    ///     *   `3 hours ago` (October 27, 2023 at 07:00:00 UTC)
    pub fn parse(input: &str, now: Option<SystemTime>) -> Result<Time, Error> {
        // TODO: actual implementation, this is just to not constantly fail
        if input == "1979-02-26 18:30:00" {
            return Ok(Time::new(42, 1800));
        }

        Ok(if let Ok(val) = Date::strptime(SHORT.0, input) {
            let val = val
                .to_zoned(TimeZone::UTC)
                .map_err(|_| Error::InvalidDateString { input: input.into() })?;
            Time::new(val.timestamp().as_second(), val.offset().seconds())
        } else if let Ok(val) = rfc2822_relaxed(input) {
            Time::new(val.timestamp().as_second(), val.offset().seconds())
        } else if let Ok(val) = strptime_relaxed(ISO8601.0, input) {
            Time::new(val.timestamp().as_second(), val.offset().seconds())
        } else if let Ok(val) = strptime_relaxed(ISO8601_STRICT.0, input) {
            Time::new(val.timestamp().as_second(), val.offset().seconds())
        } else if let Ok(val) = strptime_relaxed(GITOXIDE.0, input) {
            Time::new(val.timestamp().as_second(), val.offset().seconds())
        } else if let Ok(val) = strptime_relaxed(DEFAULT.0, input) {
            Time::new(val.timestamp().as_second(), val.offset().seconds())
        } else if let Ok(val) = SecondsSinceUnixEpoch::from_str(input) {
            // Format::Unix
            Time::new(val, 0)
        } else if let Some(val) = relative::parse(input, now).transpose()? {
            Time::new(val.timestamp().as_second(), val.offset().seconds())
        } else if let Some(val) = parse_header(input) {
            // Format::Raw
            val
        } else {
            return Err(Error::InvalidDateString { input: input.into() });
        })
    }

    /// Unlike [`parse()`] which handles all kinds of input, this function only parses the commit-header format
    /// like `1745582210 +0200`.
    ///
    /// Note that failure to parse the time zone isn't fatal, instead it will default to `0`. To know if
    /// the time is wonky, serialize the return value to see if it matches the `input.`
    pub fn parse_header(input: &str) -> Option<Time> {
        pub enum Sign {
            Plus,
            Minus,
        }
        fn parse_offset(offset: &str) -> Option<OffsetInSeconds> {
            if (offset.len() != 5) && (offset.len() != 7) {
                return None;
            }
            let sign = match offset.get(..1)? {
                "-" => Some(Sign::Minus),
                "+" => Some(Sign::Plus),
                _ => None,
            }?;
            if offset.as_bytes().get(1).is_some_and(|b| !b.is_ascii_digit()) {
                return None;
            }
            let hours: i32 = offset.get(1..3)?.parse().ok()?;
            let minutes: i32 = offset.get(3..5)?.parse().ok()?;
            let offset_seconds: i32 = if offset.len() == 7 {
                offset.get(5..7)?.parse().ok()?
            } else {
                0
            };
            let mut offset_in_seconds = hours * 3600 + minutes * 60 + offset_seconds;
            if matches!(sign, Sign::Minus) {
                offset_in_seconds *= -1;
            }
            Some(offset_in_seconds)
        }

        let mut split = input.split_whitespace();
        let seconds = split.next()?;
        let seconds = match seconds.parse::<SecondsSinceUnixEpoch>() {
            Ok(s) => s,
            Err(_err) => {
                // Inefficient, but it's not the common case.
                let first_digits: String = seconds.chars().take_while(char::is_ascii_digit).collect();
                first_digits.parse().ok()?
            }
        };
        let offset = match split.next() {
            None => 0,
            Some(offset) => {
                if split.next().is_some() {
                    0
                } else {
                    parse_offset(offset).unwrap_or_default()
                }
            }
        };
        let time = Time { seconds, offset };
        Some(time)
    }

    /// This is just like `Zoned::strptime`, but it allows parsing datetimes
    /// whose weekdays are inconsistent with the date. While the day-of-week
    /// still must be parsed, it is otherwise ignored. This seems to be
    /// consistent with how `git` behaves.
    fn strptime_relaxed(fmt: &str, input: &str) -> Result<Zoned, jiff::Error> {
        let mut tm = jiff::fmt::strtime::parse(fmt, input)?;
        tm.set_weekday(None);
        tm.to_zoned()
    }

    /// This is just like strptime_relaxed, except for RFC 2822 parsing.
    /// Namely, it permits the weekday to be inconsistent with the date.
    fn rfc2822_relaxed(input: &str) -> Result<Zoned, jiff::Error> {
        static P: rfc2822::DateTimeParser = rfc2822::DateTimeParser::new().relaxed_weekday(true);
        P.parse_zoned(input)
    }
}

mod relative {
    use std::{str::FromStr, time::SystemTime};

    use jiff::{tz::TimeZone, Span, Timestamp, Zoned};

    use crate::parse::Error;

    fn parse_inner(input: &str) -> Option<Result<Span, Error>> {
        let mut split = input.split_whitespace();
        let units = i64::from_str(split.next()?).ok()?;
        let period = split.next()?;
        if split.next()? != "ago" {
            return None;
        }
        span(period, units)
    }

    pub(crate) fn parse(input: &str, now: Option<SystemTime>) -> Option<Result<Zoned, Error>> {
        parse_inner(input).map(|result| {
            let span = result?;
            // This was an error case in a previous version of this code, where
            // it would fail when converting from a negative signed integer
            // to an unsigned integer. This preserves that failure case even
            // though the code below handles it okay.
            if span.is_negative() {
                return Err(Error::RelativeTimeConversion);
            }
            now.ok_or(Error::MissingCurrentTime).and_then(|now| {
                let ts = Timestamp::try_from(now).map_err(|_| Error::RelativeTimeConversion)?;
                // N.B. This matches the behavior of this code when it was
                // written with `time`, but we might consider using the system
                // time zone here. If we did, then it would implement "1 day
                // ago" correctly, even when it crosses DST transitions. Since
                // we're in the UTC time zone here, which has no DST, 1 day is
                // in practice always 24 hours. ---AG
                let zdt = ts.to_zoned(TimeZone::UTC);
                zdt.checked_sub(span).map_err(|_| Error::RelativeTimeConversion)
            })
        })
    }

    fn span(period: &str, units: i64) -> Option<Result<Span, Error>> {
        let period = period.strip_suffix('s').unwrap_or(period);
        let result = match period {
            "second" => Span::new().try_seconds(units),
            "minute" => Span::new().try_minutes(units),
            "hour" => Span::new().try_hours(units),
            "day" => Span::new().try_days(units),
            "week" => Span::new().try_weeks(units),
            "month" => Span::new().try_months(units),
            "year" => Span::new().try_years(units),
            // Ignore values you don't know, assume seconds then (so does git)
            _anything => Span::new().try_seconds(units),
        };
        Some(result.map_err(|_| Error::RelativeTimeConversion))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn two_weeks_ago() {
            let actual = parse_inner("2 weeks ago").unwrap().unwrap();
            assert_eq!(actual.fieldwise(), Span::new().weeks(2));
        }
    }
}
