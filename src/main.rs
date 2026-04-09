mod cli;

use clap::Parser;

fn main() {
    // Load .env file if present (before clap parses, so env vars are available)
    let _ = dotenvy::dotenv();

    let _cli = cli::Cli::parse();
    println!("Hello, world!");
}
