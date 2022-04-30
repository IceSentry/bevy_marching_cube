cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --out-name wasm_marching_cube --out-dir wasm/target --target web target/wasm32-unknown-unknown/release/bevy_marching_cube.wasm