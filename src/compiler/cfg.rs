use std::collections::{BTreeMap, BTreeSet};

use crate::compiler::op::{Op, decode};

const ROM_START: u16 = 0x200;

#[derive(Debug, Clone)]
pub(super) struct DecodedInstruction {
    pub(super) addr: u16,
    pub(super) op: Op,
}

#[derive(Debug, Clone)]
pub(super) struct DecodedBlock {
    pub(super) id: u32,
    pub(super) start_addr: u16,
    pub(super) instructions: Vec<DecodedInstruction>,
}

fn rom_end_addr(rom: &[u8]) -> u16 {
    ROM_START.wrapping_add((rom.len() as u16) & !1)
}

fn is_addr_in_rom(addr: u16, rom_end: u16) -> bool {
    addr >= ROM_START && addr < rom_end
}

fn is_block_terminator(op: &Op) -> bool {
    matches!(
        op,
        Op::Ret
            | Op::Jp { .. }
            | Op::Call { .. }
            | Op::SeByte { .. }
            | Op::SneByte { .. }
            | Op::SeReg { .. }
            | Op::SneReg { .. }
            | Op::Skp { .. }
            | Op::Sknp { .. }
            | Op::JpV0 { .. }
            | Op::LdVxK { .. }
    )
}

fn decode_rom(rom: &[u8]) -> Vec<DecodedInstruction> {
    rom.chunks_exact(2)
        .enumerate()
        .map(|(index, bytes)| {
            let opcode = ((bytes[0] as u16) << 8) | bytes[1] as u16;
            DecodedInstruction {
                addr: ROM_START + (index as u16) * 2,
                op: decode(opcode),
            }
        })
        .collect()
}

pub(super) fn build_blocks(rom: &[u8]) -> Vec<DecodedBlock> {
    let decoded_rom = decode_rom(rom);
    if decoded_rom.is_empty() {
        return Vec::new();
    }

    let rom_end = rom_end_addr(rom);
    let mut leader_addrs = BTreeSet::new();
    leader_addrs.insert(ROM_START);

    for decoded in &decoded_rom {
        let current = decoded.addr;
        let next = current.wrapping_add(2);
        let skip = current.wrapping_add(4);

        match decoded.op {
            Op::Jp { nnn } | Op::Call { nnn } => {
                if is_addr_in_rom(nnn, rom_end) {
                    leader_addrs.insert(nnn);
                }
            }
            Op::SeByte { .. }
            | Op::SneByte { .. }
            | Op::SeReg { .. }
            | Op::SneReg { .. }
            | Op::Skp { .. }
            | Op::Sknp { .. } => {
                if is_addr_in_rom(skip, rom_end) {
                    leader_addrs.insert(skip);
                }
            }
            _ => {}
        }

        if is_block_terminator(&decoded.op) && is_addr_in_rom(next, rom_end) {
            leader_addrs.insert(next);
        }
    }

    let instruction_by_addr: BTreeMap<u16, DecodedInstruction> = decoded_rom
        .into_iter()
        .map(|inst| (inst.addr, inst))
        .collect();
    let ordered_leaders: Vec<u16> = leader_addrs.into_iter().collect();
    let leader_set: BTreeSet<u16> = ordered_leaders.iter().copied().collect();

    let mut blocks = Vec::new();

    for start_addr in ordered_leaders {
        if !instruction_by_addr.contains_key(&start_addr) {
            continue;
        }

        let mut instructions = Vec::new();
        let mut current_addr = start_addr;

        loop {
            let Some(decoded) = instruction_by_addr.get(&current_addr) else {
                break;
            };

            if current_addr != start_addr && leader_set.contains(&current_addr) {
                break;
            }

            instructions.push(decoded.clone());

            if is_block_terminator(&decoded.op) {
                break;
            }

            let next_addr = current_addr.wrapping_add(2);
            if !is_addr_in_rom(next_addr, rom_end) {
                break;
            }
            current_addr = next_addr;
        }

        if !instructions.is_empty() {
            blocks.push(DecodedBlock {
                id: blocks.len() as u32,
                start_addr,
                instructions,
            });
        }
    }

    blocks
}
