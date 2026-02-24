use super::Op;

pub fn decode(opcode: u16) -> Op {
    let nnn = opcode & 0x0FFF;
    let kk = (opcode & 0xFF) as u8;
    let x = ((opcode >> 8) & 0xF) as u8;
    let y = ((opcode >> 4) & 0xF) as u8;
    let n = (opcode & 0xF) as u8;

    match (opcode >> 12, x, y, n) {
        (0x0, 0, 0xE, 0x0) => Op::Cls,
        (0x0, 0, 0xC, _) => Op::ScrollDown { n },
        (0x0, 0, 0xF, 0xB) => Op::ScrollRight,
        (0x0, 0, 0xF, 0xC) => Op::ScrollLeft,
        (0x0, 0, 0xE, 0xE) => Op::Ret,
        (0x1, ..) => Op::Jp { nnn },
        (0x2, ..) => Op::Call { nnn },
        (0x3, ..) => Op::SeByte { x, kk },
        (0x4, ..) => Op::SneByte { x, kk },
        (0x5, _, _, 0) => Op::SeReg { x, y },
        (0x6, ..) => Op::LdByte { x, kk },
        (0x7, ..) => Op::AddByte { x, kk },
        (0x8, _, _, 0x0) => Op::LdReg { x, y },
        (0x8, _, _, 0x1) => Op::Or { x, y },
        (0x8, _, _, 0x2) => Op::And { x, y },
        (0x8, _, _, 0x3) => Op::Xor { x, y },
        (0x8, _, _, 0x4) => Op::AddReg { x, y },
        (0x8, _, _, 0x5) => Op::Sub { x, y },
        (0x8, _, _, 0x6) => Op::Shr { x },
        (0x8, _, _, 0x7) => Op::Subn { x, y },
        (0x8, _, _, 0xE) => Op::Shl { x },
        (0x9, _, _, 0) => Op::SneReg { x, y },
        (0xA, ..) => Op::LdI { nnn },
        (0xB, ..) => Op::JpV0 { nnn },
        (0xC, ..) => Op::Rnd { x, kk },
        (0xD, ..) => Op::Drw { x, y, n },
        (0xE, _, 0x9, 0xE) => Op::Skp { x },
        (0xE, _, 0xA, 0x1) => Op::Sknp { x },
        (0xF, _, 0x0, 0x7) => Op::LdVxDt { x },
        (0xF, _, 0x0, 0xA) => Op::LdVxK { x },
        (0xF, _, 0x1, 0x5) => Op::LdDtVx { x },
        (0xF, _, 0x1, 0x8) => Op::LdStVx { x },
        (0xF, _, 0x1, 0xE) => Op::AddI { x },
        (0xF, _, 0x2, 0x9) => Op::LdF { x },
        (0xF, _, 0x3, 0x3) => Op::Bcd { x },
        (0xF, _, 0x5, 0x5) => Op::StoreRegs { x },
        (0xF, _, 0x6, 0x5) => Op::LoadRegs { x },
        (0xF, _, 0x7, 0x5) => Op::StoreRpl { x },
        (0xF, _, 0x8, 0x5) => Op::LoadRpl { x },
        _ => Op::Unknown { opcode },
    }
}
