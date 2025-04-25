use std::time::SystemTime;

use gix_date::Time;

#[test]
fn special_time_is_ok_for_now() {
    assert_eq!(
        gix_date::parse("1979-02-26 18:30:00", Some(SystemTime::now())).unwrap(),
        Time {
            seconds: 42,
            offset: 1800,
        }
    );
}

#[test]
fn short() {
    assert_eq!(
        gix_date::parse("1979-02-26", Some(SystemTime::now())).unwrap(),
        Time {
            seconds: 288835200,
            offset: 0,
        },
        "could not parse with SHORT format"
    );
}

#[test]
fn rfc2822() {
    assert_eq!(
        gix_date::parse("Thu, 18 Aug 2022 12:45:06 +0800", None).unwrap(),
        Time {
            seconds: 1660797906,
            offset: 28800,
        },
    );
}

#[test]
fn git_rfc2822() {
    let expected = Time {
        seconds: 1659329106,
        offset: 28800,
    };
    assert_eq!(
        gix_date::parse("Thu, 1 Aug 2022 12:45:06 +0800", None).unwrap(),
        expected,
    );
    assert_eq!(
        gix_date::parse("Thu,  1 Aug 2022 12:45:06 +0800", None).unwrap(),
        expected,
    );
}

#[test]
fn raw() -> gix_testtools::Result {
    assert_eq!(
        gix_date::parse("1660874655 +0800", None)?,
        Time {
            seconds: 1660874655,
            offset: 28800,
        },
    );

    assert_eq!(
        gix_date::parse("1112911993 +0100", None)?,
        Time {
            seconds: 1112911993,
            offset: 3600,
        },
    );

    assert_eq!(
        gix_date::parse("1313584730 +051800", None)?,
        Time {
            seconds: 1313584730,
            offset: 19080,
        },
        "seconds for time-offsets work as well"
    );

    assert_eq!(
        gix_date::parse("1313584730 +051842", None)?,
        Time {
            seconds: 1313584730,
            offset: 19122,
        },
    );

    let expected = Time {
        seconds: 1660874655,
        offset: -28800,
    };
    for date_str in [
        "1660874655 -0800",
        "1660874655 -0800  ",
        "  1660874655 -0800",
        "  1660874655 -0800  ",
        "  1660874655  -0800  ",
        "1660874655\t-0800",
        "1660874655\t-080000",
    ] {
        assert_eq!(gix_date::parse_header(date_str), Some(expected));
    }
    Ok(())
}

#[test]
fn bad_raw() {
    for bad_date_str in [
        "123456 !0600",
        "123456 +060",
        "123456 -060",
        "123456 +06000",
        "123456 +10030",
        "123456 06000",
        "123456  0600",
        "123456 +0600 extra",
        "123456+0600",
        "123456 + 600",
    ] {
        assert_eq!(
            gix_date::parse_header(bad_date_str),
            Some(Time {
                seconds: 123456,
                offset: 0
            }),
            "{bad_date_str}: invalid offsets default to zero (like in git2), and Git ignores them mostly"
        );
    }
}

#[test]
fn double_negation_in_offset() {
    let actual = gix_date::parse_header("1288373970 --700").unwrap();
    assert_eq!(
        actual,
        gix_date::Time {
            seconds: 1288373970,
            offset: 0,
        },
        "double-negation isn't special, it's considered malformed"
    );

    assert_eq!(
        actual.to_string(),
        "1288373970 +0000",
        "serialization is lossy as offset couldn't be parsed"
    );
}

#[test]
fn git_default() {
    assert_eq!(
        gix_date::parse("Thu Aug 8 12:45:06 2022 +0800", None).unwrap(),
        Time {
            seconds: 1659933906,
            offset: 28800,
        },
    );
}

#[test]
fn invalid_dates_can_be_produced_without_current_time() {
    assert!(matches!(
        gix_date::parse("foobar", None).unwrap_err(),
        gix_date::parse::Error::InvalidDateString { input } if input == "foobar"
    ));
}

mod relative {
    use std::time::SystemTime;

    use jiff::{ToSpan, Zoned};
    use pretty_assertions::assert_eq;

    #[test]
    fn large_offsets() {
        gix_date::parse("999999999999999 weeks ago", Some(std::time::UNIX_EPOCH)).ok();
    }

    #[test]
    fn large_offsets_do_not_panic() {
        assert!(matches!(
            gix_date::parse("9999999999 weeks ago", Some(std::time::UNIX_EPOCH)),
            Err(gix_date::parse::Error::RelativeTimeConversion)
        ));
    }

    #[test]
    fn offset_leading_to_before_unix_epoch_can_be_represented() {
        let date = gix_date::parse("1 second ago", Some(std::time::UNIX_EPOCH)).unwrap();
        assert_eq!(date.seconds, -1);
    }

    #[test]
    fn various() {
        let now = SystemTime::now();
        let cases = [
            ("5 seconds ago", 5.seconds()),
            ("12345 florx ago", 12_345.seconds()), // Anything parses as seconds
            ("5 minutes ago", 5.minutes()),
            ("5 hours ago", 5.hours()),
            ("5 days ago", 5.days()),
            ("3 weeks ago", 3.weeks()),
            ("21 days ago", 21.days()),              // 3 weeks
            ("504 hours ago", 504.hours()),          // 3 weeks
            ("30240 minutes ago", 30_240.minutes()), // 3 weeks
            ("2 months ago", 2.months()),
            ("1460 hours ago", 1460.hours()),        // 2 months
            ("87600 minutes ago", 87_600.minutes()), // 2 months
            ("14 weeks ago", 14.weeks()),
            ("98 days ago", 98.days()),                // 14 weeks
            ("2352 hours ago", 2352.hours()),          // 14 weeks
            ("141120 minutes ago", 141_120.minutes()), // 14 weeks
            ("5 months ago", 5.months()),
            ("3650 hours ago", 3650.hours()),          // 5 months
            ("219000 minutes ago", 219_000.minutes()), // 5 months
            ("26 weeks ago", 26.weeks()),
            ("182 days ago", 182.days()),              // 26 weeks
            ("4368 hours ago", 4368.hours()),          // 26 weeks
            ("262080 minutes ago", 262_080.minutes()), // 26 weeks
            ("8 months ago", 8.months()),
            ("5840 hours ago", 5840.hours()),          // 8 months
            ("350400 minutes ago", 350_400.minutes()), // 8 months
            ("38 weeks ago", 38.weeks()),
            ("266 days ago", 266.days()),              // 38 weeks
            ("6384 hours ago", 6384.hours()),          // 38 weeks
            ("383040 minutes ago", 383_040.minutes()), // 38 weeks
            ("11 months ago", 11.months()),
            ("8030 hours ago", 8030.hours()),          // 11 months
            ("481800 minutes ago", 481_800.minutes()), // 11 months
            ("14 months ago", 14.months()),            // "1 year, 2 months ago" not yet supported.
            ("21 months ago", 21.months()),            // "1 year, 9 months ago" not yet supported.
            ("2 years ago", 2.years()),
            ("20 years ago", 20.years()),
            ("630720000 seconds ago", 630_720_000.seconds()), // 20 years
        ];

        let cases_with_times = cases.map(|(input, _)| {
            let time = gix_date::parse(input, Some(now)).expect("relative time string should parse to a Time");
            (input, time)
        });
        assert_eq!(
            cases_with_times.map(|(_, time)| time.offset),
            cases_with_times.map(|_| 0),
            "They don't pick up local time"
        );

        let expected = cases.map(|(input, span)| {
            let expected = Zoned::new(
                now.try_into().expect("system time is representable"),
                // As relative dates are always UTC in Git, we do the same, and must
                // compare to UTC as well or else time might be off due to daylight savings, etc.
                jiff::tz::TimeZone::UTC,
            )
            // account for the loss of precision when creating `Time` with seconds
            .round(
                jiff::ZonedRound::new()
                    .smallest(jiff::Unit::Second)
                    .mode(jiff::RoundMode::Trunc),
            )
            .expect("test needs to truncate current timestamp to seconds")
            .saturating_sub(span)
            .timestamp();

            (input, expected)
        });
        let actual = cases_with_times.map(|(input, time)| {
            let actual = jiff::Timestamp::from_second(time.seconds)
                .expect("seconds obtained from a Time should convert to Timestamp");
            (input, actual)
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn various_examples() {
        #[rustfmt::skip]
        let expected = [
        // ### 1. SHORT Format
            "2018-12-24",
            "1950-01-01",
            "1970-01-01",
            "2024-12-31",

        // ### 2. RFC2822 Format
            "Thu, 18 Aug 2022 12:45:06 +0800",
            "Mon Oct 27 10:30:00 2023 -0800",

        // ### 3. GIT_RFC2822 Format
            "Thu, 8 Aug 2022 12:45:06 +0800",
            "Mon Oct 27 10:30:00 2023 -0800",

        // ### 4. ISO8601 Format
            "2022-08-17 22:04:58 +0200",
            "1970-01-01 00:00:00 -0500",

        // ### 5. ISO8601_STRICT Format
            "2022-08-17T21:43:13+08:00",

        // ### 6. UNIX Timestamp (Seconds Since Epoch)
            "123456789",
            "0",
            "-100",
            "1700000000",


        // ### 7. RAW Format
            "1745582210 +0200",
            "1660874655 +0800",
            "-1660874655 +0800",
        // ### 8. GITOXIDE Format
            "Thu Sep 04 2022 10:45:06 -0400",
            "Mon Oct 27 2023 10:30:00 +0000",
        // ### 9. DEFAULT Format
            "Thu Sep 4 10:45:06 2022 -0400",
            "Mon Oct 27 10:30:00 2023 +0000",
        ];
        for date in expected {
            _ = gix_date::parse(date, None)
                .unwrap_or_else(|err| unreachable!("{date}: all examples can be parsed: {err}"));
        }
    }
}

/// Various cases the fuzzer found
mod fuzz {
    #[test]
    fn invalid_but_does_not_cause_panic() {
        for input in ["-9999-1-1", "7	-𬞋", "5 ڜ-09", "-4 week ago Z", "8960609 day ago"] {
            gix_date::parse(input, Some(std::time::UNIX_EPOCH)).ok();
        }
    }
}
