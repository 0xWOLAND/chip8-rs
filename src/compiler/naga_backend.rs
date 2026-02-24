const INTERPRETER_WGSL: &str = include_str!("../shaders/chip8.compute.wgsl");

pub(super) fn emit_wgsl() -> String {
    let module = naga::front::wgsl::parse_str(INTERPRETER_WGSL)
        .expect("generated WGSL failed to parse");
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    validator
        .validate(&module)
        .expect("generated WGSL failed naga validation");
    INTERPRETER_WGSL.to_string()
}
