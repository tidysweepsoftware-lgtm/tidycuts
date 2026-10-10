//! Validates the WGSL with naga so shader mistakes fail `cargo test` with a
//! readable message instead of a GPU panic at runtime.

#[test]
fn shaders_parse_and_validate() {
    let src = tidycuts_render::shader_source();
    let module = match naga::front::wgsl::parse_str(src) {
        Ok(m) => m,
        Err(e) => panic!("{}", e.emit_to_string(src)),
    };
    let mut v = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    );
    if let Err(e) = v.validate(&module) {
        panic!("{}", e.emit_to_string(src));
    }
}
