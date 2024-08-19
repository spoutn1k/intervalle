use clap::Parser;

#[derive(Parser)]
struct Args {
    #[clap(required = true, value_parser = intervalle::TimeSpec::parse, allow_hyphen_values = true)]
    intervalle: intervalle::TimeSpec,
}

fn main() {
    let cli = Args::parse();

    println!("Parsed: {:?}", cli.intervalle)
}
