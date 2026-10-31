use clap::{Parser, Subcommand, ValueEnum};
use oxidye::{theme, Mode, Paths, Seed};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Apply fixed Qt/GTK themes with OKLCH accents")]
struct Cli {
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    /// Apply fixed appearance, icons, VS Code theme and an optional generated accent.
    Theme {
        #[arg(long, value_enum)]
        mode: Option<Mode>,
        #[arg(long)]
        accent: Option<Seed>,
        #[arg(long, value_enum, default_value_t=Format::Text)]
        format: Format,
        #[arg(long)]
        validator_dir: Option<PathBuf>,
        /// Skip GSettings and reload notifications for isolated testing.
        #[arg(long)]
        no_notify: bool,
    },
    /// Restore the files and preferences saved before the first fixed-theme apply.
    RestoreTheme {
        #[arg(long)]
        no_notify: bool,
    },
}
#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Text,
    Json,
}
fn run() -> Result<(), String> {
    match Cli::parse().command {
        Action::Theme { mode, accent, format, validator_dir, no_notify } => {
            let applied = theme::apply(&Paths::environment()?, mode, accent, &theme::validator_directory(validator_dir)?, !no_notify)?;
            match format {
                Format::Json => println!("{}", serde_json::to_string(&applied).map_err(|e| e.to_string())?),
                Format::Text => println!("Applied {} Qt/GTK. Accent: {}. Icons: {}.", applied.mode.as_str(), applied.seed, applied.icon_theme),
            }
            if !no_notify {
                eprintln!("Reopen GTK and Qt applications for complete styling and icon updates.");
            }
        }
        Action::RestoreTheme { no_notify } => {
            theme::restore(&Paths::environment()?, !no_notify)?;
            println!("Restored the configuration saved before the first fixed-theme apply.");
        }
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("oxidye: {error}");
        std::process::exit(1);
    }
}
