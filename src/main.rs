use clap::Parser;

fn main() {
    let cli = kuk::cli::Cli::parse();

    if let Err(e) = kuk::cli::run(cli) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
