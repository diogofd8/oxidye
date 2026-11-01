use clap::{Parser, Subcommand, ValueEnum};
use oxidye::{matugen::Scheme, theme, Mode, Paths, Seed};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Apply fixed OKLCH or image-derived Matugen Qt/GTK themes")]
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
    /// Generate an image theme with Matugen, or switch its rendered appearance.
    Matugen {
        #[arg(long)]
        image: Option<PathBuf>,
        #[arg(long, value_enum)]
        mode: Option<Mode>,
        /// Generation scheme; defaults to scheme-tonal-spot for a new image.
        #[arg(long, value_enum, requires = "image")]
        scheme: Option<Scheme>,
        #[arg(long, value_enum, default_value_t=Format::Text)]
        format: Format,
        #[arg(long)]
        validator_dir: Option<PathBuf>,
        /// Skip GSettings and reload notifications for isolated testing.
        #[arg(long)]
        no_notify: bool,
    },
    /// Restore the files and preferences saved before the first theme application.
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
fn show(applied: &theme::Applied, format: Format, no_notify: bool) -> Result<(), String> {
    match format {
        Format::Json => println!("{}", serde_json::to_string(applied).map_err(|e| e.to_string())?),
        Format::Text => println!("Applied {} {}Qt/GTK. Accent: {}. Icons: {}.", applied.mode.as_str(), if applied.source == theme::Source::Matugen { "Matugen " } else { "" }, applied.seed, applied.icon_theme),
    }
    if !no_notify {
        eprintln!("Reopen GTK and Qt applications for complete styling and icon updates.");
    }
    Ok(())
}
fn run() -> Result<(), String> {
    match Cli::parse().command {
        Action::Theme { mode, accent, format, validator_dir, no_notify } => {
            let applied = theme::apply(&Paths::environment()?, mode, accent, &theme::validator_directory(validator_dir)?, !no_notify)?;
            show(&applied, format, no_notify)?;
        }
        Action::Matugen { image, mode, scheme, format, validator_dir, no_notify } => {
            let applied = theme::apply_matugen(&Paths::environment()?, mode, image.as_deref(), scheme, &theme::validator_directory(validator_dir)?, !no_notify)?;
            show(&applied, format, no_notify)?;
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
