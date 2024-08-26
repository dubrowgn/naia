cfg_if! {
    if #[cfg(all(target_arch = "wasm32", feature = "wbindgen"))] {
        mod wasm_bindgen;
        pub use self::wasm_bindgen::*;
    }
    else {
        mod native;
        pub use self::native::*;
    }
}
