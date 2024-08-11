# `since-rs`

This small utility crate implements the format for the `--since` argument of `systemd`'s `journalctl`:
```
-S, --since=, -U, --until=
   Start showing entries on or newer than the specified date, or on or
   older than the specified date, respectively. Date specifications
   should be of the format "2012-10-30 18:17:16". If the time part is
   omitted, "00:00:00" is assumed. If only the seconds component is
   omitted, ":00" is assumed. If the date component is omitted, the
   current day is assumed. Alternatively the strings "yesterday",
   "today", "tomorrow" are understood, which refer to 00:00:00 of the
   day before the current day, the current day, or the day after the
   current day, respectively.  "now" refers to the current time.
   Finally, relative times may be specified, prefixed with "-" or "+",
   referring to times before or after the current time, respectively.
```

## Usage

The crate defines a `TimeSpec` enum that represents the time argument. You can parse a string with the `TimeSpec::parse` function and let the program determine the point in time for 'now' (as the values `today`, `tomorrow` and `yesterday` need) or use the `TimeSpec::parse_with_anchor` method to supply your own `time::PrimitiveDateTime` to use for calculations. 

```rs
use since::TimeSpec;

fn main() {
    let timespec = std::env::args().skip(1).next().unwrap();

    match TimeSpec::parse(&timespec) {
        Ok(t) => println!("{t:?}"),
        Err(e) => eprintln!("{e}"),
    }
}
```
