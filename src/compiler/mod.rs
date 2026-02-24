mod naga_backend;
pub mod op;

#[cfg(test)]
mod tests;

pub use op::{Op, decode};

pub fn compile() -> String {
    naga_backend::emit_wgsl()
}
