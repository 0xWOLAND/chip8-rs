use std::collections::HashMap;

use crate::compiler::cfg::{DecodedBlock, build_blocks};
use crate::compiler::op::Op;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EffectIr {
    ClearDisplay,
    SetRegImm { x: u8, kk: u8 },
    AddRegImm { x: u8, kk: u8 },
    SetRegReg { x: u8, y: u8 },
    Or { x: u8, y: u8 },
    And { x: u8, y: u8 },
    Xor { x: u8, y: u8 },
    AddReg { x: u8, y: u8 },
    SubReg { x: u8, y: u8 },
    Shr { x: u8 },
    SubnReg { x: u8, y: u8 },
    Shl { x: u8 },
    SetI { nnn: u16 },
    RandMask { x: u8, kk: u8 },
    Draw { x: u8, y: u8, n: u8 },
    LoadDelayToV { x: u8 },
    SetDelayFromV { x: u8 },
    SetSoundFromV { x: u8 },
    AddIFromV { x: u8 },
    SetIFont { x: u8 },
    Bcd { x: u8 },
    StoreRegs { x: u8 },
    LoadRegs { x: u8 },
    Unknown { opcode: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TerminatorIr {
    Goto {
        target: u32,
    },
    Call {
        target: u32,
        ret: u32,
    },
    Return,
    BranchEqImm {
        x: u8,
        kk: u8,
        then_target: u32,
        else_target: u32,
    },
    BranchNeImm {
        x: u8,
        kk: u8,
        then_target: u32,
        else_target: u32,
    },
    BranchEqReg {
        x: u8,
        y: u8,
        then_target: u32,
        else_target: u32,
    },
    BranchNeReg {
        x: u8,
        y: u8,
        then_target: u32,
        else_target: u32,
    },
    BranchKeyPressed {
        x: u8,
        then_target: u32,
        else_target: u32,
    },
    BranchKeyNotPressed {
        x: u8,
        then_target: u32,
        else_target: u32,
    },
    WaitKey {
        x: u8,
        on_found: u32,
        on_not_found: u32,
    },
    JumpV0 {
        base: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BlockIr {
    pub(crate) id: u32,
    pub(crate) effects: Vec<EffectIr>,
    pub(crate) term: TerminatorIr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProgramIr {
    pub(crate) entry_block: u32,
    pub(crate) addr_to_block: Vec<(u16, u32)>,
    pub(crate) blocks: Vec<BlockIr>,
}

fn block_for(addr: u16, fallback: u32, addr_map: &HashMap<u16, u32>) -> u32 {
    addr_map.get(&addr).copied().unwrap_or(fallback)
}

fn lower_effect(op: &Op) -> Option<EffectIr> {
    match *op {
        Op::Cls => Some(EffectIr::ClearDisplay),
        Op::LdByte { x, kk } => Some(EffectIr::SetRegImm { x, kk }),
        Op::AddByte { x, kk } => Some(EffectIr::AddRegImm { x, kk }),
        Op::LdReg { x, y } => Some(EffectIr::SetRegReg { x, y }),
        Op::Or { x, y } => Some(EffectIr::Or { x, y }),
        Op::And { x, y } => Some(EffectIr::And { x, y }),
        Op::Xor { x, y } => Some(EffectIr::Xor { x, y }),
        Op::AddReg { x, y } => Some(EffectIr::AddReg { x, y }),
        Op::Sub { x, y } => Some(EffectIr::SubReg { x, y }),
        Op::Shr { x } => Some(EffectIr::Shr { x }),
        Op::Subn { x, y } => Some(EffectIr::SubnReg { x, y }),
        Op::Shl { x } => Some(EffectIr::Shl { x }),
        Op::LdI { nnn } => Some(EffectIr::SetI { nnn }),
        Op::Rnd { x, kk } => Some(EffectIr::RandMask { x, kk }),
        Op::Drw { x, y, n } => Some(EffectIr::Draw { x, y, n }),
        Op::LdVxDt { x } => Some(EffectIr::LoadDelayToV { x }),
        Op::LdDtVx { x } => Some(EffectIr::SetDelayFromV { x }),
        Op::LdStVx { x } => Some(EffectIr::SetSoundFromV { x }),
        Op::AddI { x } => Some(EffectIr::AddIFromV { x }),
        Op::LdF { x } => Some(EffectIr::SetIFont { x }),
        Op::Bcd { x } => Some(EffectIr::Bcd { x }),
        Op::StoreRegs { x } => Some(EffectIr::StoreRegs { x }),
        Op::LoadRegs { x } => Some(EffectIr::LoadRegs { x }),
        Op::Unknown { opcode } => Some(EffectIr::Unknown { opcode }),
        _ => None,
    }
}

fn lower_terminator(
    op: &Op,
    block: &DecodedBlock,
    addr_map: &HashMap<u16, u32>,
    next: u16,
) -> Option<TerminatorIr> {
    let skip = next.wrapping_add(2);
    let next_block = block_for(next, block.id, addr_map);
    let skip_block = block_for(skip, block.id, addr_map);

    match *op {
        Op::Ret => Some(TerminatorIr::Return),
        Op::Jp { nnn } => Some(TerminatorIr::Goto {
            target: block_for(nnn, block.id, addr_map),
        }),
        Op::Call { nnn } => Some(TerminatorIr::Call {
            target: block_for(nnn, block.id, addr_map),
            ret: next_block,
        }),
        Op::SeByte { x, kk } => Some(TerminatorIr::BranchEqImm {
            x,
            kk,
            then_target: skip_block,
            else_target: next_block,
        }),
        Op::SneByte { x, kk } => Some(TerminatorIr::BranchNeImm {
            x,
            kk,
            then_target: skip_block,
            else_target: next_block,
        }),
        Op::SeReg { x, y } => Some(TerminatorIr::BranchEqReg {
            x,
            y,
            then_target: skip_block,
            else_target: next_block,
        }),
        Op::SneReg { x, y } => Some(TerminatorIr::BranchNeReg {
            x,
            y,
            then_target: skip_block,
            else_target: next_block,
        }),
        Op::Skp { x } => Some(TerminatorIr::BranchKeyPressed {
            x,
            then_target: skip_block,
            else_target: next_block,
        }),
        Op::Sknp { x } => Some(TerminatorIr::BranchKeyNotPressed {
            x,
            then_target: skip_block,
            else_target: next_block,
        }),
        Op::LdVxK { x } => Some(TerminatorIr::WaitKey {
            x,
            on_found: next_block,
            on_not_found: block.id,
        }),
        Op::JpV0 { nnn } => Some(TerminatorIr::JumpV0 { base: nnn }),
        _ => None,
    }
}

fn lower_block(block: &DecodedBlock, addr_map: &HashMap<u16, u32>) -> BlockIr {
    let mut effects = Vec::new();

    for inst in &block.instructions[..block.instructions.len().saturating_sub(1)] {
        if let Some(effect) = lower_effect(&inst.op) {
            effects.push(effect);
        }
    }

    let last = block
        .instructions
        .last()
        .expect("decoded block must have at least one instruction");
    let next_addr = last.addr.wrapping_add(2);

    let term = if let Some(term) = lower_terminator(&last.op, block, addr_map, next_addr) {
        term
    } else {
        if let Some(effect) = lower_effect(&last.op) {
            effects.push(effect);
        }
        TerminatorIr::Goto {
            target: block_for(next_addr, block.id, addr_map),
        }
    };

    BlockIr {
        id: block.id,
        effects,
        term,
    }
}

pub(crate) fn lower(rom: &[u8]) -> ProgramIr {
    let decoded_blocks = build_blocks(rom);
    let addr_map_hash: HashMap<u16, u32> = decoded_blocks
        .iter()
        .map(|block| (block.start_addr, block.id))
        .collect();

    let mut addr_to_block: Vec<(u16, u32)> = addr_map_hash.iter().map(|(k, v)| (*k, *v)).collect();
    addr_to_block.sort_by_key(|(addr, _)| *addr);

    let blocks = decoded_blocks
        .iter()
        .map(|block| lower_block(block, &addr_map_hash))
        .collect();

    ProgramIr {
        entry_block: 0,
        addr_to_block,
        blocks,
    }
}
