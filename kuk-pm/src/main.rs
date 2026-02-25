use clap::Parser;

fn main() {
    let cli = kuk_pm::cli::Cli::parse();
    if let Err(e) = kuk_pm::cli::run(cli) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
