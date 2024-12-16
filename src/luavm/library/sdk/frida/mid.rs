#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use frida_gum::{
        interceptor::{Interceptor, ProbeListener},
        NativePointer,
    };

    struct MyProbeListener;

    impl ProbeListener for MyProbeListener {
        fn on_hit(&mut self, context: frida_gum::interceptor::InvocationContext) {
            eprintln!("rcx: {:x}", context.cpu_context().rcx());
            eprintln!("rdx: {:x}", context.cpu_context().rdx());
        }
    }

    static GUM: LazyLock<frida_gum::Gum> = LazyLock::new(frida_gum::Gum::obtain);

    #[inline(never)]
    extern "C" fn test_add(a: i32, b: i32) -> i32 {
        a + b
    }

    #[test]
    fn frida_hook_mid() {
        let mut interceptor = Interceptor::obtain(&GUM);

        let _listener = interceptor
            .attach_instruction(NativePointer(test_add as _), &mut MyProbeListener)
            .unwrap();

        eprintln!("test_add(1, 2) = {}", test_add(1, 2));
    }
}
