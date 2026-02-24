use chip8_frag::{AppResult, Chip8App, DEFAULT_SHADER_FILE};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "chip8-frag")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Compile {
        out: Option<PathBuf>,
    },
    Visualize {
        rom: PathBuf,
    },
}

fn run() -> AppResult<()> {
    match Cli::parse().command {
        Command::Compile { out } => {
            let shader_path = Chip8App::compile_shader_file(out.as_deref())?;
            println!("{}", shader_path.display());
        }
        Command::Visualize { rom } => {
            let _ = Chip8App::visualize_rom_file(&rom)?;
            println!("{}", DEFAULT_SHADER_FILE);
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
