use time::ext::NumericalDuration;
#[allow(unused_imports)]
use winnow::{
    ascii::{digit1, space0, space1},
    combinator::{alt, eof, fail, opt, preceded, repeat, separated_pair, seq, terminated},
    error::{AddContext, ContextError, ParseError, ParserError, StrContext, StrContextValue},
    prelude::*,
    token::{literal, take_until, take_while},
};

#[derive(PartialEq, Debug)]
pub enum TimeSpec {
    After(time::PrimitiveDateTime),
    Before(time::PrimitiveDateTime),
    Point(time::PrimitiveDateTime),
}

fn yesterday(
    anchor: time::PrimitiveDateTime,
) -> Result<time::PrimitiveDateTime, time::error::ComponentRange> {
    Ok(anchor.date().midnight().checked_sub(1.days()).unwrap())
}

fn tomorrow(
    anchor: time::PrimitiveDateTime,
) -> Result<time::PrimitiveDateTime, time::error::ComponentRange> {
    Ok(anchor.date().midnight().checked_add(1.days()).unwrap())
}

macro_rules! digits {
    ($len:expr, $dest:ty) => {
        digit1
            .verify(|s: &str| s.len() == $len)
            .try_map(str::parse::<$dest>)
    };
}

macro_rules! date {
    () => {
        (
            digits!(4, u16),
            preceded("-", digits!(2, u8)),
            preceded("-", digits!(2, u8)),
        )
            .try_map(|(year, month, day)| {
                time::Date::from_calendar_date(year as i32, time::Month::try_from(month)?, day)
            })
            .map(|d| d.midnight())
    };
}

macro_rules! time {
    () => {
        (
            digits!(2, u8),
            preceded(":", digits!(2, u8)),
            opt(preceded(":", digits!(2, u8))),
        )
            .try_map(|(hour, min, sec)| time::Time::from_hms(hour, min, sec.unwrap_or(0)))
    };
}

impl TimeSpec {
    pub fn parse(timespec: &str) -> Result<TimeSpec, Box<dyn std::error::Error>> {
        let now: time::OffsetDateTime = std::time::SystemTime::now().into();
        TimeSpec::parse_with_anchor(
            timespec,
            time::PrimitiveDateTime::new(now.date(), now.time()),
        )
    }

    pub fn parse_with_anchor(
        timespec: &str,
        anchor: time::PrimitiveDateTime,
    ) -> Result<TimeSpec, Box<dyn std::error::Error>> {
        let out: Result<Self, ParseError<&str, ContextError>> = (
            opt(alt(("+", "-"))),
            alt((
                literal("today").value(anchor.date().midnight()),
                literal("yesterday").try_map(|_| yesterday(anchor)),
                literal("tomorrow").try_map(|_| tomorrow(anchor)),
                separated_pair(date!(), " ", time!())
                    .map(|(pdate, ptime)| pdate.replace_time(ptime)),
                date!(),
                time!().map(|ptime| anchor.replace_time(ptime)),
            )),
        )
            .map(|(modifier, dtime)| match modifier {
                Some("+") => Self::After(dtime),
                Some("-") => Self::Before(dtime),
                None => Self::Point(dtime),
                _ => unreachable!(),
            })
            .parse(timespec);

        Ok(out.map_err(|e| e.to_string())?)
    }
}

#[test]
fn test_today() {
    let target = time::Date::from_calendar_date(2023, time::Month::November, 11)
        .unwrap()
        .midnight();

    let anchor = target.replace_time(time::Time::from_hms(12, 20, 45).unwrap());

    let parsed = TimeSpec::parse_with_anchor("today", anchor).unwrap();

    assert_eq!(parsed, TimeSpec::Point(target))
}

#[test]
fn test_yesterday() {
    let target = time::Date::from_calendar_date(2023, time::Month::November, 10)
        .unwrap()
        .midnight();

    let anchor = time::Date::from_calendar_date(2023, time::Month::November, 11)
        .unwrap()
        .midnight()
        .replace_time(time::Time::from_hms(12, 20, 45).unwrap());

    let parsed = TimeSpec::parse_with_anchor("yesterday", anchor).unwrap();

    assert_eq!(parsed, TimeSpec::Point(target))
}

#[test]
fn test_tomorrow() {
    let target = time::Date::from_calendar_date(2023, time::Month::November, 12)
        .unwrap()
        .midnight();

    let anchor = time::Date::from_calendar_date(2023, time::Month::November, 11)
        .unwrap()
        .midnight()
        .replace_time(time::Time::from_hms(12, 20, 45).unwrap());

    let parsed = TimeSpec::parse_with_anchor("tomorrow", anchor).unwrap();

    assert_eq!(parsed, TimeSpec::Point(target))
}

#[test]
fn test_date_time() {
    let target = time::Date::from_calendar_date(2024, time::Month::August, 08)
        .unwrap()
        .midnight()
        .replace_time(time::Time::from_hms(14, 10, 11).unwrap());

    let anchor = time::Date::from_calendar_date(2023, time::Month::November, 11)
        .unwrap()
        .midnight()
        .replace_time(time::Time::from_hms(12, 20, 45).unwrap());

    let parsed = TimeSpec::parse_with_anchor("2024-08-08 14:10:11", anchor).unwrap();

    assert_eq!(parsed, TimeSpec::Point(target))
}

#[test]
fn test_date_time_no_sec() {
    let target = time::Date::from_calendar_date(2024, time::Month::August, 08)
        .unwrap()
        .midnight()
        .replace_time(time::Time::from_hms(14, 10, 00).unwrap());

    let anchor = time::Date::from_calendar_date(2023, time::Month::November, 11)
        .unwrap()
        .midnight()
        .replace_time(time::Time::from_hms(12, 20, 45).unwrap());

    let parsed = TimeSpec::parse_with_anchor("2024-08-08 14:10", anchor).unwrap();

    assert_eq!(parsed, TimeSpec::Point(target))
}

#[test]
fn test_date() {
    let target = time::Date::from_calendar_date(2024, time::Month::August, 08)
        .unwrap()
        .midnight();

    let anchor = time::Date::from_calendar_date(2023, time::Month::November, 11)
        .unwrap()
        .midnight()
        .replace_time(time::Time::from_hms(12, 20, 45).unwrap());

    let parsed = TimeSpec::parse_with_anchor("2024-08-08", anchor).unwrap();

    assert_eq!(parsed, TimeSpec::Point(target))
}

#[test]
fn test_time() {
    let target = time::Date::from_calendar_date(2024, time::Month::August, 08)
        .unwrap()
        .midnight()
        .replace_time(time::Time::from_hms(15, 27, 59).unwrap());

    let anchor = target.replace_time(time::Time::from_hms(12, 20, 45).unwrap());

    let parsed = TimeSpec::parse_with_anchor("15:27:59", anchor).unwrap();

    assert_eq!(parsed, TimeSpec::Point(target))
}

#[test]
fn test_time_no_sec() {
    let target = time::Date::from_calendar_date(2024, time::Month::August, 08)
        .unwrap()
        .midnight()
        .replace_time(time::Time::from_hms(15, 28, 00).unwrap());

    let anchor = target.replace_time(time::Time::from_hms(12, 20, 45).unwrap());

    let parsed = TimeSpec::parse_with_anchor("15:28", anchor).unwrap();

    assert_eq!(parsed, TimeSpec::Point(target))
}

#[test]
fn test_before_time_no_sec() {
    let target = time::Date::from_calendar_date(2024, time::Month::August, 08)
        .unwrap()
        .midnight()
        .replace_time(time::Time::from_hms(15, 28, 00).unwrap());

    let anchor = target.replace_time(time::Time::from_hms(12, 20, 45).unwrap());

    let parsed = TimeSpec::parse_with_anchor("-15:28", anchor).unwrap();

    assert_eq!(parsed, TimeSpec::Before(target))
}

#[test]
fn test_after_time_no_sec() {
    let target = time::Date::from_calendar_date(2024, time::Month::August, 08)
        .unwrap()
        .midnight()
        .replace_time(time::Time::from_hms(15, 28, 00).unwrap());

    let anchor = target.replace_time(time::Time::from_hms(12, 20, 45).unwrap());

    let parsed = TimeSpec::parse_with_anchor("+15:28", anchor).unwrap();

    assert_eq!(parsed, TimeSpec::After(target))
}

#[test]
fn test_too_early() {
    let anchor = time::PrimitiveDateTime::new(
        time::Date::from_calendar_date(1970, time::Month::January, 01).unwrap(),
        time::Time::from_hms(12, 20, 45).unwrap(),
    );

    let parsed = TimeSpec::parse_with_anchor("yesterday", anchor);

    assert!(parsed.is_err())
}
