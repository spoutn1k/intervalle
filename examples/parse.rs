use since::TimeSpec;

fn main() {
    let timespec = std::env::args().skip(1).next().unwrap();

    match TimeSpec::parse(&timespec) {
        Ok(t) => println!("{t:?}"),
        Err(e) => eprintln!("{e}"),
    }
}
