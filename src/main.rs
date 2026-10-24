mod cli;
mod config;
mod fsops;
mod paint;
mod tui;
mod util;

use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    match cli::run(cli) {
        Ok(code) => ExitCode::from(code as u8),
        Err(err) => {
            eprintln!("{} {}", paint::red("linkman:"), err);
            ExitCode::from(1)
        }
    }
}
