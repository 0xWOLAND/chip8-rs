use chip8_rs::{AppError, Chip8App};
use std::path::Path;

fn run() -> Result<(), AppError> {
    let rom = std::env::args().nth(1).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "usage: chip8-rs <rom.ch8>")
    })?;
    Chip8App::run(Path::new(&rom))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
