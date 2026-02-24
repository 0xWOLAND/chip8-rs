mod cfg;
mod ir;
mod naga_backend;
pub mod op;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use op::{Op, decode};

pub fn compile(_rom: &[u8]) -> String {
    naga_backend::emit_wgsl()
}
