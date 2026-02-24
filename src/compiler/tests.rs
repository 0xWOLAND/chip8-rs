use super::{compile, decode, op::Op};

#[test]
fn decode_cls() {
    assert_eq!(decode(0x00E0), Op::Cls);
}

#[test]
fn decode_ret() {
    assert_eq!(decode(0x00EE), Op::Ret);
}

#[test]
fn decode_schip_scroll_ops() {
    assert_eq!(decode(0x00C4), Op::ScrollDown { n: 4 });
    assert_eq!(decode(0x00FB), Op::ScrollRight);
    assert_eq!(decode(0x00FC), Op::ScrollLeft);
}

#[test]
fn decode_jp() {
    assert_eq!(decode(0x1ABC), Op::Jp { nnn: 0xABC });
}

#[test]
fn decode_call() {
    assert_eq!(decode(0x2456), Op::Call { nnn: 0x456 });
}

#[test]
fn decode_se_byte() {
    assert_eq!(decode(0x3A42), Op::SeByte { x: 0xA, kk: 0x42 });
}

#[test]
fn decode_sne_byte() {
    assert_eq!(decode(0x4B10), Op::SneByte { x: 0xB, kk: 0x10 });
}

#[test]
fn decode_se_reg() {
    assert_eq!(decode(0x5120), Op::SeReg { x: 1, y: 2 });
}

#[test]
fn decode_ld_byte() {
    assert_eq!(decode(0x6AFF), Op::LdByte { x: 0xA, kk: 0xFF });
}

#[test]
fn decode_add_byte() {
    assert_eq!(decode(0x7305), Op::AddByte { x: 3, kk: 5 });
}

#[test]
fn decode_alu_ops() {
    assert_eq!(decode(0x8120), Op::LdReg { x: 1, y: 2 });
    assert_eq!(decode(0x8121), Op::Or { x: 1, y: 2 });
    assert_eq!(decode(0x8122), Op::And { x: 1, y: 2 });
    assert_eq!(decode(0x8123), Op::Xor { x: 1, y: 2 });
    assert_eq!(decode(0x8124), Op::AddReg { x: 1, y: 2 });
    assert_eq!(decode(0x8125), Op::Sub { x: 1, y: 2 });
    assert_eq!(decode(0x8126), Op::Shr { x: 1 });
    assert_eq!(decode(0x8127), Op::Subn { x: 1, y: 2 });
    assert_eq!(decode(0x812E), Op::Shl { x: 1 });
}

#[test]
fn decode_sne_reg() {
    assert_eq!(decode(0x9340), Op::SneReg { x: 3, y: 4 });
}

#[test]
fn decode_ld_i() {
    assert_eq!(decode(0xA123), Op::LdI { nnn: 0x123 });
}

#[test]
fn decode_jp_v0() {
    assert_eq!(decode(0xB300), Op::JpV0 { nnn: 0x300 });
}

#[test]
fn decode_rnd() {
    assert_eq!(decode(0xC5AA), Op::Rnd { x: 5, kk: 0xAA });
}

#[test]
fn decode_drw() {
    assert_eq!(decode(0xD235), Op::Drw { x: 2, y: 3, n: 5 });
}

#[test]
fn decode_skp() {
    assert_eq!(decode(0xE19E), Op::Skp { x: 1 });
}

#[test]
fn decode_sknp() {
    assert_eq!(decode(0xE1A1), Op::Sknp { x: 1 });
}

#[test]
fn decode_fx_ops() {
    assert_eq!(decode(0xF207), Op::LdVxDt { x: 2 });
    assert_eq!(decode(0xF30A), Op::LdVxK { x: 3 });
    assert_eq!(decode(0xF415), Op::LdDtVx { x: 4 });
    assert_eq!(decode(0xF518), Op::LdStVx { x: 5 });
    assert_eq!(decode(0xF61E), Op::AddI { x: 6 });
    assert_eq!(decode(0xF729), Op::LdF { x: 7 });
    assert_eq!(decode(0xF833), Op::Bcd { x: 8 });
    assert_eq!(decode(0xF955), Op::StoreRegs { x: 9 });
    assert_eq!(decode(0xFA65), Op::LoadRegs { x: 0xA });
    assert_eq!(decode(0xF375), Op::StoreRpl { x: 3 });
    assert_eq!(decode(0xF485), Op::LoadRpl { x: 4 });
}

#[test]
fn decode_unknown() {
    assert_eq!(decode(0x0000), Op::Unknown { opcode: 0x0000 });
    assert_eq!(decode(0x5121), Op::Unknown { opcode: 0x5121 });
}

#[test]
fn compile_has_block_dispatch() {
    let wgsl = compile();
    assert!(wgsl.contains("block_id"));
    assert!(wgsl.contains("fn execute_cycle()"));
    assert!(wgsl.contains("let op ="));
}

#[test]
fn compile_conditional_emits_two_successors() {
    let wgsl = compile();
    assert!(wgsl.contains("pc + 4u"));
    assert!(wgsl.contains("if reg_read(x) == kk"));
}

#[test]
fn compile_call_pushes_return_block() {
    let wgsl = compile();
    assert!(wgsl.contains("stack_write(vm.sp, next_pc)"));
    assert!(wgsl.contains("stack_read(vm.sp)"));
}

#[test]
fn compile_wait_key_spins_on_same_block() {
    let wgsl = compile();
    assert!(wgsl.contains("var found = false"));
    assert!(wgsl.contains("if !found { next_pc = pc; }"));
}

#[test]
fn compile_dynamic_jump_uses_addr_map() {
    let wgsl = compile();
    assert!(wgsl.contains("next_pc = (nnn + reg_read(0u)) & 0xFFEu;"));
}

#[test]
fn compile_drw_baked_height() {
    let wgsl = compile();
    assert!(wgsl.contains("for (var row = 0u; row < n; row++)"));
    assert!(wgsl.contains("draw_sprite(vx: u32, vy: u32, n: u32)"));
    assert!(wgsl.contains(">= 64u"));
    assert!(wgsl.contains(">= 32u"));
}

#[test]
fn compile_drw_zero_height_emits_schip_16x16() {
    let wgsl = compile();
    assert!(wgsl.contains("< 16u"));
    assert!(wgsl.contains("sprite_word"));
    assert!(wgsl.contains("0x8000u"));
}
