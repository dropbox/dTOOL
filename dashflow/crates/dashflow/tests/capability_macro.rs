#[dashflow::capability("test_capability", test_capability_2)]
struct AnnotatedType;

#[test]
fn capability_macro_compiles_and_is_noop() {
    let _ = AnnotatedType;
}
