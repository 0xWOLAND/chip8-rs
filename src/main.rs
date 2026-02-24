use chip8_frag::chip8::Chip8;

fn main() {
    let mut chip = Chip8::new();

    let rom = std::env::args()
        .nth(1)
        .expect("usage: chip8-frag <rom>");

    let data = std::fs::read(&rom).expect("failed to read rom");
    chip.load(&data);

    println!("loaded {} bytes from {rom}", data.len());
}
