set -ex

build_guest() {
  # use `-Zbuild-std` to get the wasm64 target primarily, otherwise just
  # build the guest in the same mode.
  #
  # Note that the `-Z` flag here is an attempt to fix an otherwise
  # seemingly infinite loop in LLVM optimizations. Unsure what's happening
  # there.
  RUSTFLAGS=-Znew-llvm-pass-manager=no \
  cargo +nightly build -p guest -Z build-std=panic_abort,std --release "$@"
}

build_guest --target wasm64-unknown-unknown
build_guest --target wasm32-unknown-unknown

cargo run --release -- "$@"
#node --experimental-wasm-memory64 run.js "$@"
