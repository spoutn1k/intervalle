use time::ext::NumericalDuration;
use winnow::{
    ascii::digit1,
    combinator::{alt, cut_err, opt, preceded, separated_pair},
    error::{ContextError, ParseError, StrContext, StrContextValue},
    prelude::*,
    token::literal,
};

#[derive(PartialEq, Debug)]
pub enum TimeSpec {
    After(time::PrimitiveDateTime),
    Before(time::PrimitiveDateTime),
    Point(time::PrimitiveDateTime),
}

fn yesterday(anchor: time::PrimitiveDateTime) -> time::PrimitiveDateTime {
    anchor
        .date()
        .midnight()
        .checked_sub(1.days())
        .expect("Unreacheable, we allow 4 digit years and the library supports i32")
}

fn tomorrow(anchor: time::PrimitiveDateTime) -> time::PrimitiveDateTime {
    anchor
        .date()
        .midnight()
        .checked_add(1.days())
        .expect("Unreacheable, we allow 4 digit years and the library supports i32")
}

macro_rules! digits {
    ($len:expr, $dest:ty) => {
        digit1
            .verify(|s: &str| s.len() == $len)
            .try_map(str::parse::<$dest>)
            .context(StrContext::Label("digit count"))
    };
}

macro_rules! date {
    () => {
        (
            digits!(4, u16),
            preceded(
                cut_err("-")
                    .context(StrContext::Label("date delimiter"))
                    .context(StrContext::Expected(StrContextValue::CharLiteral('-'))),
                digits!(2, u8),
            ),
            preceded(
                cut_err("-")
                    .context(StrContext::Label("date delimiter"))
                    .context(StrContext::Expected(StrContextValue::CharLiteral('-'))),
                digits!(2, u8),
            ),
        )
            .try_map(|(year, month, day)| {
                time::Date::from_calendar_date(year as i32, time::Month::try_from(month)?, day)
            })
            .map(|d| d.midnight())
            .context(StrContext::Label("date format"))
    };
}

macro_rules! time {
    () => {
        (
            digits!(2, u8),
            preceded(
                cut_err(":")
                    .context(StrContext::Label("time delimiter"))
                    .context(StrContext::Expected(StrContextValue::CharLiteral(':'))),
                cut_err(digits!(2, u8)),
            ),
            opt(preceded(
                literal(":")
                    .context(StrContext::Label("time delimiter"))
                    .context(StrContext::Expected(StrContextValue::CharLiteral(':'))),
                cut_err(digits!(2, u8)),
            )),
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
                literal("yesterday").value(yesterday(anchor)),
                literal("tomorrow").value(tomorrow(anchor)),
                separated_pair(
                    date!(),
                    literal(" ").context(StrContext::Expected(StrContextValue::CharLiteral(' '))),
                    cut_err(time!()).context(StrContext::Label("time")),
                )
                .map(|(pdate, ptime)| pdate.replace_time(ptime))
                .context(StrContext::Label("time_and_date")),
                date!(),
                time!().map(|ptime| anchor.replace_time(ptime)),
            )),
        )
            .context(StrContext::Label("timespec"))
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
