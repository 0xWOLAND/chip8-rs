use chip8_frag::{AppResult, Chip8App, DEFAULT_FRAMES};
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
        rom: PathBuf,
        out: Option<PathBuf>,
    },
    Visualize {
        rom: PathBuf,
        #[arg(default_value_t = DEFAULT_FRAMES)]
        frames: u32,
    },
}

fn run() -> AppResult<()> {
    match Cli::parse().command {
        Command::Compile { rom, out } => {
            let shader_path = Chip8App::compile_rom_file(&rom, out.as_deref())?;
            println!("{}", shader_path.display());
        }
        Command::Visualize { rom, frames } => {
            let image_path = Chip8App::visualize_rom_file(&rom, frames)?;
            println!("{}", image_path.display());
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
