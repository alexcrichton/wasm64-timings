const fs = require('fs');
const wasmToValidate = fs.readFileSync(process.argv[2]);

function run(name, file, to_ptr, from_ptr) {
  const m = new WebAssembly.Module(fs.readFileSync(file));
  const i = new WebAssembly.Instance(m);
  const malloc = i.exports.malloc;
  const free = i.exports.free;
  const memory = i.exports.memory;
  const validate = i.exports.validate;

  const now = performance.now();
  const ptrlen = to_ptr(wasmToValidate.length);
  const ptr = malloc(ptrlen);
  (new Uint8Array(memory.buffer)).set(wasmToValidate, from_ptr(ptr));
  validate(ptr, ptrlen);
  free(ptr, ptrlen);
  return performance.now() - now;
}

const dur32 = run('wasm32', 'target/wasm32-unknown-unknown/release/guest.wasm', i => i, i => i);
const dur64 = run('wasm64', 'target/wasm64-unknown-unknown/release/guest.wasm', BigInt, Number)

console.log('wasm32', dur32.toFixed(2))
console.log('wasm64', dur64.toFixed(2))

console.log(`+${((dur64 - dur32) / dur32 * 100).toFixed(2)}%`)
