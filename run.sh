set -ex
RUSTC=$HOME/code/rust/build/aarch64-unknown-linux-gnu/stage1/bin/rustc \
  cargo +nightly build -p guest --target wasm64-unknown-unknown --release
cargo build -p guest --target wasm32-unknown-unknown --release
cargo run --release -- "$@"
node --experimental-wasm-memory64 run.js "$@"
