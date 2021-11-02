use std::env;
use std::time::{Duration, Instant};
use wasmtime::*;

fn main() -> anyhow::Result<()> {
    let input = env::args().nth(1).unwrap();
    let input = std::fs::read(&input)?;
    let mut config = Config::new();
    config.wasm_memory64(true);
    let engine = Engine::new(&config)?;

    let mut store = Store::new(&engine, ());

    macro_rules! run {
        ($file:expr, $ptr:ty) => {{
            let module = Module::from_file(&engine, $file).unwrap();
            let instance = Instance::new(&mut store, &module, &[])?;
            let malloc = instance.get_typed_func::<$ptr, $ptr, _>(&mut store, "malloc")?;
            let free = instance.get_typed_func::<($ptr, $ptr), (), _>(&mut store, "free")?;
            let wat2wasm =
                instance.get_typed_func::<($ptr, $ptr), $ptr, _>(&mut store, "wat2wasm")?;
            let wasm_ptr = instance.get_typed_func::<$ptr, $ptr, _>(&mut store, "wasm_ptr")?;
            let wasm_len = instance.get_typed_func::<$ptr, $ptr, _>(&mut store, "wasm_len")?;
            let wasm_free = instance.get_typed_func::<$ptr, (), _>(&mut store, "wasm_free")?;
            let mem = instance.get_memory(&mut store, "memory").unwrap();

            let start = Instant::now();
            let len = input.len().try_into().unwrap();
            let ptr = malloc.call(&mut store, len)?;
            mem.data_mut(&mut store)[ptr.try_into().unwrap()..][..input.len()]
                .copy_from_slice(&input);
            let wasm_obj = wat2wasm.call(&mut store, (ptr, len))?;
            free.call(&mut store, (ptr, len))?;

            let ret_ptr = wasm_ptr.call(&mut store, wasm_obj)?;
            let ret_len = wasm_len.call(&mut store, wasm_obj)?;

            let wasm = mem.data(&store)[ret_ptr.try_into().unwrap()..]
                [..ret_len.try_into().unwrap()]
                .to_vec();
            wasm_free.call(&mut store, wasm_obj)?;
            (wasm, start.elapsed())
        }};
    }

    let (wasm_from_native, native_dur) = {
        let start = Instant::now();
        let wasm = wat::parse_bytes(&input).unwrap();
        (wasm, start.elapsed())
    };
    println!("native: {:?}", native_dur);

    let print_time = |name: &str, dur: Duration| {
        let (neg, diff) = if dur > native_dur {
            (false, dur - native_dur)
        } else {
            (true, native_dur - dur)
        };
        let pct = diff.as_nanos() as f64 / native_dur.as_nanos() as f64;
        println!(
            "{} time: {:?} ({}{:.02}% native)",
            name,
            dur,
            if neg { "-" } else { "+" },
            pct * 100.,
        );
    };

    let (wasm_from_32, dur) = run!("target/wasm32-unknown-unknown/release/guest.wasm", u32);
    print_time("wasm32", dur);
    assert_eq!(wasm_from_native, wasm_from_32);

    let (wasm_from_64, dur) = run!("target/wasm64-unknown-unknown/release/guest.wasm", u64);
    print_time("wasm64", dur);
    assert_eq!(wasm_from_32, wasm_from_64);

    Ok(())
}
