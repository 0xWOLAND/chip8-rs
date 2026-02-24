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

fn print_visualize_expectations(rom: &std::path::Path, cycles_per_frame: u32) {
    println!("+--------------------------------------------------------------+");
    println!("|                        CHIP-8 VISUALIZE                      |");
    println!("+--------------------------------------------------------------+");
    println!("| ROM: {:<56}|", rom.display());
    println!("| CYCLES/FRAME: {:<47}|", cycles_per_frame.max(1));
    println!("| EXPECTED: live, continuously updating WebGPU window          |");
    println!("| CONTROLS: 1 2 3 4 / Q W E R / A S D F / Z X C V / ESC quit |");
    println!("+--------------------------------------------------------------+");
}

fn run() -> AppResult<()> {
    match Cli::parse().command {
        Command::Compile { rom, out } => {
            let shader_path = Chip8App::compile_rom_file(&rom, out.as_deref())?;
            println!("{}", shader_path.display());
        }
        Command::Visualize { rom, frames } => {
            print_visualize_expectations(&rom, frames);
            let output_path = Chip8App::visualize_rom_file(&rom, frames)?;
            println!("{}", output_path.display());
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
