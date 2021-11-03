set -ex
RUSTFLAGS=-Clink-args=--max-memory=104857600 \
RUSTC=$HOME/code/rust/build/x86_64-unknown-linux-gnu/stage1/bin/rustc \
  cargo +nightly build -p guest --target wasm64-unknown-unknown --release
cargo build -p guest --target wasm32-unknown-unknown --release
cargo run --release -- "$@"
node --experimental-wasm-memory64 run.js "$@"
