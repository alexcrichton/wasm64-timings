const fs = require('fs');
const input = fs.readFileSync(process.argv[2]);

function run(name, file, to_ptr, from_ptr) {
  const m = new WebAssembly.Module(fs.readFileSync(file));
  const i = new WebAssembly.Instance(m);
  const { malloc, free, memory, validate, wat2wasm, wasm_free } = i.exports;

  const now = performance.now();
  const ptrlen = to_ptr(input.length);
  const ptr = malloc(ptrlen);
  (new Uint8Array(memory.buffer)).set(input, from_ptr(ptr));
  const obj = wat2wasm(ptr, ptrlen);
  free(ptr, ptrlen);
  wasm_free(ptr);
  return performance.now() - now;
}

const dur32 = run('wasm32', 'target/wasm32-unknown-unknown/release/guest.wasm', i => i, i => i);
const dur64 = run('wasm64', 'target/wasm64-unknown-unknown/release/guest.wasm', BigInt, Number)

console.log('wasm32', dur32.toFixed(2))
console.log('wasm64', dur64.toFixed(2))

console.log(`+${((dur64 - dur32) / dur32 * 100).toFixed(2)}%`)
