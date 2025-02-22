use std::error::Error;
use time::{
    ext::NumericalDuration, OffsetDateTime, PrimitiveDateTime as DateTime, Time, UtcOffset,
};
use winnow::{
    ascii::digit1,
    combinator::{alt, cut_err, opt, preceded, separated_pair},
    error::{ParseError, StrContext, StrContextValue},
    prelude::*,
    token::literal,
    ModalResult,
};

#[derive(Debug)]
pub enum IntervalleError {
    ParseError(String, String, usize),
}

impl<C> From<ParseError<&str, C>> for IntervalleError
where
    C: std::fmt::Display,
{
    fn from(ce: ParseError<&str, C>) -> Self {
        Self::ParseError(
            format!("{}", ce.inner()).replace("\n", ", "),
            String::from(*ce.input()),
            ce.offset(),
        )
    }
}

impl std::fmt::Display for IntervalleError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            IntervalleError::ParseError(info, input, offset) => {
                write!(f, "\n    |\n{offset:3} | {input}\n    | ")?;
                for _ in 0..*offset {
                    write!(f, " ")?;
                }
                write!(f, "^ {info}")
            }
        }
    }
}

impl Error for IntervalleError {}

#[derive(PartialEq, Debug, Clone)]
pub enum TimeSpec {
    After(DateTime),
    Before(DateTime),
    Point(DateTime),
}

fn yesterday(anchor: DateTime) -> DateTime {
    anchor
        .date()
        .midnight()
        .checked_sub(1.days())
        .expect("Unreacheable, we allow 4 digit years and the library supports i32")
}

fn tomorrow(anchor: DateTime) -> DateTime {
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
    /// Figuring out the system's local timezone
    fn local_offset() -> Result<UtcOffset, Box<dyn Error>> {
        let time_zone_local = tz::TimeZone::local()?
            .find_current_local_time_type()?
            .ut_offset();

        UtcOffset::from_whole_seconds(time_zone_local).map_err(|e| e.into())
    }

    pub fn parse(timespec: &str) -> Result<TimeSpec, IntervalleError> {
        let now =
            OffsetDateTime::now_utc().to_offset(Self::local_offset().unwrap_or(UtcOffset::UTC));
        let now = DateTime::new(now.date(), now.time());
        let time_range = TimeRange::parser
            .parse(timespec)
            .map_err(IntervalleError::from)?;

        Ok(time_range.evaluate(now))
    }
}

#[derive(Debug, Clone)]
enum TimeRange {
    Before(TimeRef),
    After(TimeRef),
    Point(TimeRef),
}

#[derive(Debug, Clone)]
enum TimeRef {
    Today,
    Yesterday,
    Tomorrow,
    DateTime(DateTime),
    Date(DateTime),
    Time(Time),
}

impl TimeRef {
    fn evaluate(&self, now: DateTime) -> DateTime {
        match self {
            TimeRef::Today => now.date().midnight(),
            TimeRef::Yesterday => yesterday(now),
            TimeRef::Tomorrow => tomorrow(now),
            TimeRef::DateTime(dt) => *dt,
            TimeRef::Date(d) => *d,
            TimeRef::Time(t) => now.date().midnight().replace_time(*t),
        }
    }
}

impl TimeRange {
    fn parser(timespec: &mut &str) -> ModalResult<TimeRange> {
        (
            opt(alt(("+", "-"))),
            alt((
                literal("today").value(TimeRef::Today),
                literal("yesterday").value(TimeRef::Yesterday),
                literal("tomorrow").value(TimeRef::Tomorrow),
                separated_pair(
                    date!(),
                    literal(" ").context(StrContext::Expected(StrContextValue::CharLiteral(' '))),
                    cut_err(time!()).context(StrContext::Label("time")),
                )
                .map(|(pdate, ptime)| TimeRef::DateTime(pdate.replace_time(ptime)))
                .context(StrContext::Label("time_and_date")),
                date!().map(TimeRef::Date),
                time!().map(TimeRef::Time),
            )),
        )
            .context(StrContext::Label("timespec"))
            .map(|(modifier, dtime)| match modifier {
                Some("+") => TimeRange::After(dtime),
                Some("-") => TimeRange::Before(dtime),
                None => TimeRange::Point(dtime),
                _ => unreachable!(),
            })
            .parse_next(timespec)
    }

    fn evaluate(&self, now: DateTime) -> TimeSpec {
        match self {
            TimeRange::Before(dtime) => TimeSpec::Before(dtime.evaluate(now)),
            TimeRange::After(dtime) => TimeSpec::After(dtime.evaluate(now)),
            TimeRange::Point(dtime) => TimeSpec::Point(dtime.evaluate(now)),
        }
    }
}

#[test]
fn test_parse_today() {
    insta::assert_debug_snapshot!(TimeRange::parser.parse("today").unwrap());
}

#[test]
fn test_evaluate_today() {
    let target = time::Date::from_calendar_date(2023, time::Month::November, 11)
        .unwrap()
        .midnight();

    let anchor = target.replace_time(time::Time::from_hms(12, 20, 45).unwrap());
    let range = TimeRange::Point(TimeRef::Today);

    assert_eq!(range.evaluate(anchor), TimeSpec::Point(target))
}

#[test]
fn test_parse_yesterday() {
    insta::assert_debug_snapshot!(TimeRange::parser.parse("yesterday").unwrap())
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
    let range = TimeRange::Point(TimeRef::Yesterday);

    assert_eq!(range.evaluate(anchor), TimeSpec::Point(target))
}

#[test]
fn test_parse_tomorrow() {
    insta::assert_debug_snapshot!(TimeRange::parser.parse("tomorrow").unwrap())
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
    let range = TimeRange::Point(TimeRef::Tomorrow);

    assert_eq!(range.evaluate(anchor), TimeSpec::Point(target))
}

#[test]
fn test_parse_date_time() {
    insta::assert_debug_snapshot!(TimeRange::parser.parse("2024-08-08 14:10:11").unwrap())
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

    let range = TimeRange::Point(TimeRef::DateTime(
        time::Date::from_calendar_date(2024, time::Month::August, 08)
            .unwrap()
            .midnight()
            .replace_time(time::Time::from_hms(14, 10, 11).unwrap()),
    ));

    assert_eq!(range.evaluate(anchor), TimeSpec::Point(target))
}

#[test]
fn test_parse_date_time_no_sec() {
    insta::assert_debug_snapshot!(TimeRange::parser.parse("2024-08-08 14:10").unwrap())
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

    let range = TimeRange::Point(TimeRef::DateTime(
        time::Date::from_calendar_date(2024, time::Month::August, 08)
            .unwrap()
            .midnight()
            .replace_time(time::Time::from_hms(14, 10, 00).unwrap()),
    ));

    assert_eq!(range.evaluate(anchor), TimeSpec::Point(target))
}

#[test]
fn test_parse_date() {
    insta::assert_debug_snapshot!(TimeRange::parser.parse("2024-08-08").unwrap())
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

    let range = TimeRange::Point(TimeRef::DateTime(
        time::Date::from_calendar_date(2024, time::Month::August, 08)
            .unwrap()
            .midnight(),
    ));

    assert_eq!(range.evaluate(anchor), TimeSpec::Point(target))
}

#[test]
fn test_parse_time() {
    insta::assert_debug_snapshot!(TimeRange::parser.parse("15:28:59").unwrap())
}

#[test]
fn test_time() {
    let target = time::Date::from_calendar_date(2024, time::Month::August, 08)
        .unwrap()
        .midnight()
        .replace_time(time::Time::from_hms(15, 28, 59).unwrap());

    let anchor = target.replace_time(time::Time::from_hms(12, 20, 45).unwrap());

    let range = TimeRange::Point(TimeRef::Time(time::Time::from_hms(15, 28, 59).unwrap()));

    assert_eq!(range.evaluate(anchor), TimeSpec::Point(target))
}

#[test]
fn test_parse_time_no_sec() {
    insta::assert_debug_snapshot!(TimeRange::parser.parse("15:28").unwrap())
}

#[test]
fn test_time_no_sec() {
    let target = time::Date::from_calendar_date(2024, time::Month::August, 08)
        .unwrap()
        .midnight()
        .replace_time(time::Time::from_hms(15, 28, 00).unwrap());

    let anchor = target.replace_time(time::Time::from_hms(12, 20, 45).unwrap());

    let range = TimeRange::Point(TimeRef::Time(time::Time::from_hms(15, 28, 00).unwrap()));

    assert_eq!(range.evaluate(anchor), TimeSpec::Point(target))
}

#[test]
fn test_parse_before_time_no_sec() {
    insta::assert_debug_snapshot!(TimeRange::parser.parse("-15:28").unwrap())
}

#[test]
fn test_parse_after_time_no_sec() {
    insta::assert_debug_snapshot!(TimeRange::parser.parse("+15:28").unwrap())
}
