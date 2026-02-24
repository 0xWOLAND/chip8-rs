// CHIP-8 interpreter shader 

struct VmState {
    block_id: u32, 
    i_reg: u32,
    sp: u32,
    delay_timer: u32,
    sound_timer: u32,
    rng_state: u32,
    v: array<u32, 4>,
    rpl: array<u32, 2>,
    stack: array<u32, 8>,
    memory: array<u32, 1024>,
}

@group(0) @binding(0) var<storage, read_write> vm: VmState;
@group(0) @binding(1) var<storage, read_write> display: array<u32, 2048>;
@group(0) @binding(2) var<storage, read> keypad: array<u32, 16>;

const CYCLES_PER_FRAME: u32 = 1u;

fn mem_read(addr: u32) -> u32 {
    let a = addr & 0xFFFu;
    let word_idx = a >> 2u;
    let byte_idx = a & 3u;
    return (vm.memory[word_idx] >> (byte_idx * 8u)) & 0xFFu;
}

fn mem_write(addr: u32, val: u32) {
    let a = addr & 0xFFFu;
    let word_idx = a >> 2u;
    let byte_idx = a & 3u;
    let shift = byte_idx * 8u;
    let mask = ~(0xFFu << shift);
    vm.memory[word_idx] = (vm.memory[word_idx] & mask) | ((val & 0xFFu) << shift);
}

fn reg_read(idx: u32) -> u32 {
    let word_idx = idx >> 2u;
    let byte_idx = idx & 3u;
    return (vm.v[word_idx] >> (byte_idx * 8u)) & 0xFFu;
}

fn reg_write(idx: u32, val: u32) {
    let word_idx = idx >> 2u;
    let byte_idx = idx & 3u;
    let shift = byte_idx * 8u;
    let mask = ~(0xFFu << shift);
    vm.v[word_idx] = (vm.v[word_idx] & mask) | ((val & 0xFFu) << shift);
}

fn rpl_read(idx: u32) -> u32 {
    let word_idx = idx >> 2u;
    let byte_idx = idx & 3u;
    return (vm.rpl[word_idx] >> (byte_idx * 8u)) & 0xFFu;
}

fn rpl_write(idx: u32, val: u32) {
    let word_idx = idx >> 2u;
    let byte_idx = idx & 3u;
    let shift = byte_idx * 8u;
    let mask = ~(0xFFu << shift);
    vm.rpl[word_idx] = (vm.rpl[word_idx] & mask) | ((val & 0xFFu) << shift);
}

fn stack_read(idx: u32) -> u32 {
    let word_idx = idx >> 1u;
    let half_idx = idx & 1u;
    return (vm.stack[word_idx] >> (half_idx * 16u)) & 0xFFFFu;
}

fn stack_write(idx: u32, val: u32) {
    let word_idx = idx >> 1u;
    let half_idx = idx & 1u;
    let shift = half_idx * 16u;
    let mask = ~(0xFFFFu << shift);
    vm.stack[word_idx] = (vm.stack[word_idx] & mask) | ((val & 0xFFFFu) << shift);
}

fn xorshift() -> u32 {
    var s = vm.rng_state;
    s ^= s << 13u;
    s ^= s >> 17u;
    s ^= s << 5u;
    vm.rng_state = s;
    return s;
}

fn scroll_down(rows: u32) {
    for (var y = 31i; y >= 0i; y--) {
        for (var x = 0u; x < 64u; x++) {
            let dst = u32(y) * 64u + x;
            if y >= i32(rows) {
                let src = u32(y - i32(rows)) * 64u + x;
                display[dst] = display[src];
            } else {
                display[dst] = 0u;
            }
        }
    }
}

fn scroll_right() {
    for (var y = 0u; y < 32u; y++) {
        for (var x = 63i; x >= 0i; x--) {
            let dst = y * 64u + u32(x);
            if x >= 4i {
                let src = y * 64u + u32(x - 4i);
                display[dst] = display[src];
            } else {
                display[dst] = 0u;
            }
        }
    }
}

fn scroll_left() {
    for (var y = 0u; y < 32u; y++) {
        for (var x = 0u; x < 64u; x++) {
            let dst = y * 64u + x;
            if x + 4u < 64u {
                let src = y * 64u + (x + 4u);
                display[dst] = display[src];
            } else {
                display[dst] = 0u;
            }
        }
    }
}

fn draw_sprite(vx: u32, vy: u32, n: u32) -> u32 {
    var collision = 0u;
    if n == 0u {
        for (var row = 0u; row < 16u; row++) {
            let py = vy + row;
            if py >= 32u {
                continue;
            }
            let sprite_hi = mem_read(vm.i_reg + row * 2u);
            let sprite_lo = mem_read(vm.i_reg + row * 2u + 1u);
            let sprite_word = (sprite_hi << 8u) | sprite_lo;
            for (var col = 0u; col < 16u; col++) {
                let px = vx + col;
                if px >= 64u {
                    continue;
                }
                if (sprite_word & (0x8000u >> col)) != 0u {
                    let didx = py * 64u + px;
                    if display[didx] != 0u {
                        collision = 1u;
                    }
                    display[didx] ^= 1u;
                }
            }
        }
    } else {
        for (var row = 0u; row < n; row++) {
            let py = vy + row;
            if py >= 32u {
                continue;
            }
            let sprite_byte = mem_read(vm.i_reg + row);
            for (var col = 0u; col < 8u; col++) {
                let px = vx + col;
                if px >= 64u {
                    continue;
                }
                if (sprite_byte & (0x80u >> col)) != 0u {
                    let didx = py * 64u + px;
                    if display[didx] != 0u {
                        collision = 1u;
                    }
                    display[didx] ^= 1u;
                }
            }
        }
    }
    return collision;
}

fn execute_cycle() {
    let pc = vm.block_id & 0xFFFu;
    let op = (mem_read(pc) << 8u) | mem_read(pc + 1u);
    var next_pc = (pc + 2u) & 0xFFFu;

    let top = (op >> 12u) & 0xFu;
    let nnn = op & 0x0FFFu;
    let kk = op & 0x00FFu;
    let x = (op >> 8u) & 0xFu;
    let y = (op >> 4u) & 0xFu;
    let n = op & 0xFu;

    if op == 0x00E0u {
        for (var idx = 0u; idx < 2048u; idx++) {
            display[idx] = 0u;
        }
    } else if op == 0x00EEu {
        if vm.sp > 0u {
            vm.sp -= 1u;
            next_pc = stack_read(vm.sp) & 0xFFFu;
        }
    } else if (op & 0xFFF0u) == 0x00C0u {
        scroll_down(n);
    } else if op == 0x00FBu {
        scroll_right();
    } else if op == 0x00FCu {
        scroll_left();
    } else {
        switch top {
            case 0x1u { next_pc = nnn; }
            case 0x2u {
                stack_write(vm.sp, next_pc);
                vm.sp = (vm.sp + 1u) & 0xFu;
                next_pc = nnn;
            }
            case 0x3u {
                if reg_read(x) == kk { next_pc = (pc + 4u) & 0xFFFu; }
            }
            case 0x4u {
                if reg_read(x) != kk { next_pc = (pc + 4u) & 0xFFFu; }
            }
            case 0x5u {
                if n == 0u && reg_read(x) == reg_read(y) { next_pc = (pc + 4u) & 0xFFFu; }
            }
            case 0x6u { reg_write(x, kk); }
            case 0x7u { reg_write(x, (reg_read(x) + kk) & 0xFFu); }
            case 0x8u {
                switch n {
                    case 0x0u { reg_write(x, reg_read(y)); }
                    case 0x1u { reg_write(x, reg_read(x) | reg_read(y)); }
                    case 0x2u { reg_write(x, reg_read(x) & reg_read(y)); }
                    case 0x3u { reg_write(x, reg_read(x) ^ reg_read(y)); }
                    case 0x4u {
                        let sum = reg_read(x) + reg_read(y);
                        reg_write(x, sum & 0xFFu);
                        reg_write(0xFu, u32(sum > 0xFFu));
                    }
                    case 0x5u {
                        let vx = reg_read(x);
                        let vy = reg_read(y);
                        reg_write(x, (vx - vy) & 0xFFu);
                        reg_write(0xFu, u32(vx >= vy));
                    }
                    case 0x6u {
                        let vx = reg_read(x);
                        reg_write(x, vx >> 1u);
                        reg_write(0xFu, vx & 1u);
                    }
                    case 0x7u {
                        let vx = reg_read(x);
                        let vy = reg_read(y);
                        reg_write(x, (vy - vx) & 0xFFu);
                        reg_write(0xFu, u32(vy >= vx));
                    }
                    case 0xEu {
                        let vx = reg_read(x);
                        reg_write(x, (vx << 1u) & 0xFFu);
                        reg_write(0xFu, (vx >> 7u) & 1u);
                    }
                    default {}
                }
            }
            case 0x9u {
                if n == 0u && reg_read(x) != reg_read(y) { next_pc = (pc + 4u) & 0xFFFu; }
            }
            case 0xAu { vm.i_reg = nnn; }
            case 0xBu { next_pc = (nnn + reg_read(0u)) & 0xFFEu; }
            case 0xCu { reg_write(x, xorshift() & kk); }
            case 0xDu { reg_write(0xFu, draw_sprite(reg_read(x), reg_read(y), n)); }
            case 0xEu {
                if kk == 0x9Eu {
                    if keypad[reg_read(x) & 0xFu] != 0u { next_pc = (pc + 4u) & 0xFFFu; }
                } else if kk == 0xA1u {
                    if keypad[reg_read(x) & 0xFu] == 0u { next_pc = (pc + 4u) & 0xFFFu; }
                }
            }
            case 0xFu {
                switch kk {
                    case 0x07u { reg_write(x, vm.delay_timer & 0xFFu); }
                    case 0x0Au {
                        var found = false;
                        for (var k = 0u; k < 16u; k++) {
                            if keypad[k] != 0u {
                                reg_write(x, k);
                                found = true;
                                break;
                            }
                        }
                        if !found { next_pc = pc; }
                    }
                    case 0x15u { vm.delay_timer = reg_read(x); }
                    case 0x18u { vm.sound_timer = reg_read(x); }
                    case 0x1Eu { vm.i_reg = (vm.i_reg + reg_read(x)) & 0xFFFu; }
                    case 0x29u { vm.i_reg = 0x050u + reg_read(x) * 5u; }
                    case 0x33u {
                        let val = reg_read(x);
                        mem_write(vm.i_reg, val / 100u);
                        mem_write(vm.i_reg + 1u, (val / 10u) % 10u);
                        mem_write(vm.i_reg + 2u, val % 10u);
                    }
                    case 0x55u {
                        for (var r = 0u; r <= x; r++) {
                            mem_write(vm.i_reg + r, reg_read(r));
                        }
                        vm.i_reg = (vm.i_reg + x + 1u) & 0xFFFu;
                    }
                    case 0x65u {
                        for (var r = 0u; r <= x; r++) {
                            reg_write(r, mem_read(vm.i_reg + r));
                        }
                        vm.i_reg = (vm.i_reg + x + 1u) & 0xFFFu;
                    }
                    case 0x75u {
                        for (var r = 0u; r <= min(x, 7u); r++) {
                            rpl_write(r, reg_read(r));
                        }
                    }
                    case 0x85u {
                        for (var r = 0u; r <= min(x, 7u); r++) {
                            reg_write(r, rpl_read(r));
                        }
                    }
                    default {}
                }
            }
            default {}
        }
    }

    vm.block_id = next_pc;
}

@compute @workgroup_size(1)
fn main() {
    for (var cycle = 0u; cycle < CYCLES_PER_FRAME; cycle++) {
        execute_cycle();
    }
}
