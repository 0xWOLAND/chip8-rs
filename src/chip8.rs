use rand::Rng;

pub const WIDTH: usize = 64;
pub const HEIGHT: usize = 32;
const PROGRAM_START: u16 = 0x200;
const FONT_START: u16 = 0x050;

const FONT: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

pub struct Chip8 {
    pub v: [u8; 16],
    pub i: u16,
    pub pc: u16,
    pub sp: u8,
    pub stack: [u16; 16],
    pub memory: [u8; 4096],
    pub display: [bool; WIDTH * HEIGHT],
    pub keypad: [bool; 16],
    pub delay_timer: u8,
    pub sound_timer: u8,
    rng: rand::rngs::ThreadRng,
}

impl Chip8 {
    pub fn new() -> Self {
        let mut chip = Self {
            v: [0; 16],
            i: 0,
            pc: PROGRAM_START,
            sp: 0,
            stack: [0; 16],
            memory: [0; 4096],
            display: [false; WIDTH * HEIGHT],
            keypad: [false; 16],
            delay_timer: 0,
            sound_timer: 0,
            rng: rand::rng(),
        };
        chip.memory[FONT_START as usize..][..FONT.len()].copy_from_slice(&FONT);
        chip
    }

    pub fn load(&mut self, rom: &[u8]) {
        self.memory[PROGRAM_START as usize..][..rom.len()].copy_from_slice(rom);
    }

    pub fn step(&mut self) {
        let op = self.fetch();
        self.execute(op);
        self.delay_timer = self.delay_timer.saturating_sub(1);
        self.sound_timer = self.sound_timer.saturating_sub(1);
    }

    fn fetch(&mut self) -> u16 {
        let hi = self.memory[self.pc as usize] as u16;
        let lo = self.memory[self.pc as usize + 1] as u16;
        self.pc += 2;
        (hi << 8) | lo
    }

    fn execute(&mut self, op: u16) {
        let nnn = op & 0x0FFF;
        let kk = (op & 0xFF) as u8;
        let x = ((op >> 8) & 0xF) as usize;
        let y = ((op >> 4) & 0xF) as usize;
        let n = (op & 0xF) as usize;

        match (op >> 12, x, y, n) {
            (0x0, 0, 0xE, 0x0) => self.display = [false; WIDTH * HEIGHT],
            (0x0, 0, 0xE, 0xE) => {
                self.sp -= 1;
                self.pc = self.stack[self.sp as usize];
            }
            (0x1, ..) => self.pc = nnn,
            (0x2, ..) => {
                self.stack[self.sp as usize] = self.pc;
                self.sp += 1;
                self.pc = nnn;
            }
            (0x3, ..) => {
                if self.v[x] == kk {
                    self.pc += 2;
                }
            }
            (0x4, ..) => {
                if self.v[x] != kk {
                    self.pc += 2;
                }
            }
            (0x5, _, _, 0) => {
                if self.v[x] == self.v[y] {
                    self.pc += 2;
                }
            }
            (0x6, ..) => self.v[x] = kk,
            (0x7, ..) => self.v[x] = self.v[x].wrapping_add(kk),
            (0x8, _, _, 0x0) => self.v[x] = self.v[y],
            (0x8, _, _, 0x1) => self.v[x] |= self.v[y],
            (0x8, _, _, 0x2) => self.v[x] &= self.v[y],
            (0x8, _, _, 0x3) => self.v[x] ^= self.v[y],
            (0x8, _, _, 0x4) => {
                let (result, carry) = self.v[x].overflowing_add(self.v[y]);
                self.v[x] = result;
                self.v[0xF] = carry as u8;
            }
            (0x8, _, _, 0x5) => {
                let (result, borrow) = self.v[x].overflowing_sub(self.v[y]);
                self.v[x] = result;
                self.v[0xF] = !borrow as u8;
            }
            (0x8, _, _, 0x6) => {
                let lsb = self.v[x] & 1;
                self.v[x] >>= 1;
                self.v[0xF] = lsb;
            }
            (0x8, _, _, 0x7) => {
                let (result, borrow) = self.v[y].overflowing_sub(self.v[x]);
                self.v[x] = result;
                self.v[0xF] = !borrow as u8;
            }
            (0x8, _, _, 0xE) => {
                let msb = self.v[x] >> 7;
                self.v[x] <<= 1;
                self.v[0xF] = msb;
            }
            (0x9, _, _, 0) => {
                if self.v[x] != self.v[y] {
                    self.pc += 2;
                }
            }
            (0xA, ..) => self.i = nnn,
            (0xB, ..) => self.pc = nnn.wrapping_add(self.v[0] as u16),
            (0xC, ..) => self.v[x] = self.rng.random::<u8>() & kk,
            (0xD, ..) => self.draw(x, y, n),
            (0xE, _, 0x9, 0xE) => {
                if self.keypad[self.v[x] as usize] {
                    self.pc += 2;
                }
            }
            (0xE, _, 0xA, 0x1) => {
                if !self.keypad[self.v[x] as usize] {
                    self.pc += 2;
                }
            }
            (0xF, _, 0x0, 0x7) => self.v[x] = self.delay_timer,
            (0xF, _, 0x0, 0xA) => {
                if let Some(k) = self.keypad.iter().position(|&p| p) {
                    self.v[x] = k as u8;
                } else {
                    self.pc -= 2;
                }
            }
            (0xF, _, 0x1, 0x5) => self.delay_timer = self.v[x],
            (0xF, _, 0x1, 0x8) => self.sound_timer = self.v[x],
            (0xF, _, 0x1, 0xE) => self.i = self.i.wrapping_add(self.v[x] as u16),
            (0xF, _, 0x2, 0x9) => self.i = FONT_START + self.v[x] as u16 * 5,
            (0xF, _, 0x3, 0x3) => {
                let val = self.v[x];
                let base = self.i as usize;
                self.memory[base] = val / 100;
                self.memory[base + 1] = (val / 10) % 10;
                self.memory[base + 2] = val % 10;
            }
            (0xF, _, 0x5, 0x5) => {
                let base = self.i as usize;
                self.memory[base..=base + x].copy_from_slice(&self.v[..=x]);
            }
            (0xF, _, 0x6, 0x5) => {
                let base = self.i as usize;
                self.v[..=x].copy_from_slice(&self.memory[base..=base + x]);
            }
            _ => {}
        }
    }

    fn draw(&mut self, x: usize, y: usize, n: usize) {
        let vx = self.v[x] as usize % WIDTH;
        let vy = self.v[y] as usize % HEIGHT;
        self.v[0xF] = 0;

        for row in 0..n {
            let sprite_byte = self.memory[self.i as usize + row];
            for col in 0..8 {
                if sprite_byte & (0x80 >> col) == 0 {
                    continue;
                }
                let px = (vx + col) % WIDTH;
                let py = (vy + row) % HEIGHT;
                let idx = py * WIDTH + px;
                if self.display[idx] {
                    self.v[0xF] = 1;
                }
                self.display[idx] ^= true;
            }
        }
    }
}
