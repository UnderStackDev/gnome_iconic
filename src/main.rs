mod cli;
mod core;
mod ui;

pub const APP_ID: &str = "io.github.UnderStackDev.Iconic";

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = if cli::wants_cli(&args) { cli::run(args) } else { ui::run() };
    std::process::ExitCode::from(code as u8)
}
