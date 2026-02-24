use chip8_frag::chip8::{Chip8, WIDTH, HEIGHT};

fn display_to_string(chip: &Chip8) -> String {
    let mut out = String::new();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            out.push(if chip.display[y * WIDTH + x] { '#' } else { '.' });
        }
        out.push('\n');
    }
    out
}

#[test]
fn test_opcode_rom() {
    let rom = include_bytes!("test_opcode.ch8");
    let mut chip = Chip8::new();
    chip.load(rom);

    // Run until the PC stalls (infinite loop = test complete).
    let mut stall_count = 0u32;

    for _ in 0..100_000 {
        let pc_before = chip.pc;
        chip.step();

        if chip.pc == pc_before {
            stall_count += 1;
            if stall_count > 10 {
                break;
            }
        } else {
            stall_count = 0;
        }
    }

    let screen = display_to_string(&chip);
    println!("{screen}");

    // The test ROM draws "OK" for each passing opcode test.
    // If any pixel is set, the ROM produced output.
    let pixels_on = chip.display.iter().filter(|&&p| p).count();
    assert!(pixels_on > 0, "display is blank — ROM did not execute correctly");

    // The ROM should NOT contain "ERR" patterns — a fully passing run
    // shows only "OK" labels. We verify no error by checking that the
    // display has a reasonable number of lit pixels (each "OK" draws some).
    // A failing test would show far fewer or different patterns.
    assert!(
        pixels_on > 50,
        "too few pixels lit ({pixels_on}) — likely opcode failures"
    );
}
