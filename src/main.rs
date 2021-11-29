use anyhow::Result;
use std::env;
use std::time::{Duration, Instant};
use wasmtime::*;

const WASM_VALIDATE: bool = false;

fn main() -> Result<()> {
    let input = env::args().nth(1).unwrap();
    let input = std::fs::read(&input)?;
    let mut config = Config::new();
    config.wasm_memory64(true);

    let wasm32 = "target/wasm32-unknown-unknown/release/guest.wasm";
    let wasm64 = "target/wasm64-unknown-unknown/release/guest.wasm";

    fn print_time(name: &str, dur: Duration, baselines: &[(Duration, &str)]) {
        print!("{:>20} time: {:.02?} (", name, dur,);
        for (i, (baseline, baseline_name)) in baselines.iter().enumerate() {
            if i > 0 {
                print!(", ");
            }
            let (neg, diff) = if dur > *baseline {
                (false, dur - *baseline)
            } else {
                (true, *baseline - dur)
            };
            let pct = diff.as_nanos() as f64 / baseline.as_nanos() as f64;
            print!(
                "{}{:.02}% {}",
                if neg { "-" } else { "+" },
                pct * 100.,
                baseline_name,
            );
        }
        println!(")");
    }

    // run the benchmark natively
    let native_dur = {
        let start = Instant::now();
        if WASM_VALIDATE {
            wasmparser::Validator::new().validate_all(&input).unwrap();
        } else {
            wat::parse_bytes(&input).unwrap();
        }
        start.elapsed()
    };
    print_time("native", native_dur, &[]);

    // run the 32-bit wasm with default wasmtime settings (aka no bounds checks)
    let dur32 = run::<u32>(&Config::new(), wasm32, &input)?;
    print_time("wasm32", dur32, &[(native_dur, "native")]);

    // 32-bit, bounds-checks, no spectre mitigation
    let dur32_bc_ns = unsafe {
        run::<u32>(
            Config::new()
                .static_memory_maximum_size(0)
                .cranelift_flag_set("enable_heap_access_spectre_mitigation", "false")?,
            wasm32,
            &input,
        )?
    };
    print_time(
        "wasm32-bc-ns",
        dur32_bc_ns,
        &[(native_dur, "native"), (dur32, "wasm32")],
    );

    // 32-bit, bounds checks, with spectre mitigations
    let dur32_bc = run::<u32>(Config::new().static_memory_maximum_size(0), wasm32, &input)?;
    print_time(
        "wasm32-bc",
        dur32_bc,
        &[(native_dur, "native"), (dur32, "wasm32")],
    );

    // 64-bit, bounds checks, no spectre mitigation, static heap
    let dur64_ns = unsafe {
        run::<u64>(
            Config::new()
                .wasm_memory64(true)
                .cranelift_flag_set("enable_heap_access_spectre_mitigation", "false")?,
            wasm64,
            &input,
        )?
    };
    print_time(
        "wasm64-ns",
        dur64_ns,
        &[
            (native_dur, "native"),
            (dur32, "wasm32"),
            (dur32_bc_ns, "wasm32-bc-ns"),
        ],
    );

    // 64-bit, bounds checks, no spectre mitigation, dynamic heap
    let dur64_dyn_ns = unsafe {
        run::<u64>(
            Config::new()
                .wasm_memory64(true)
                .cranelift_flag_set("enable_heap_access_spectre_mitigation", "false")?
                .static_memory_maximum_size(0),
            wasm64,
            &input,
        )?
    };
    print_time(
        "wasm64-dyn-ns",
        dur64_dyn_ns,
        &[
            (native_dur, "native"),
            (dur32, "wasm32"),
            (dur64_ns, "wasm64-ns"),
        ],
    );

    // 64-bit, bounds checks, spectre mitigation, static heap
    let dur64 = run::<u64>(Config::new().wasm_memory64(true), wasm64, &input)?;
    print_time(
        "wasm64",
        dur64,
        &[
            (native_dur, "native"),
            (dur32, "wasm32"),
            (dur64_ns, "wasm64-ns"),
        ],
    );

    // 64-bit, bounds checks, spectre mitigation, dynamic heap
    let dur64_dyn = run::<u64>(
        Config::new()
            .wasm_memory64(true)
            .static_memory_maximum_size(0),
        wasm64,
        &input,
    )?;
    print_time(
        "wasm64-dyn",
        dur64_dyn,
        &[
            (native_dur, "native"),
            (dur32, "wasm32"),
            (dur64_ns, "wasm64-ns"),
        ],
    );
    Ok(())
}

fn run<T>(config: &Config, file: &str, input: &[u8]) -> Result<Duration>
where
    T: WasmTy + Copy + TryFrom<usize, Error = std::num::TryFromIntError>,
    usize: TryFrom<T, Error = std::num::TryFromIntError>,
{
    let engine = Engine::new(config)?;
    let mut store = Store::new(&engine, ());
    let module = Module::from_file(&engine, file).unwrap();
    let instance = Instance::new(&mut store, &module, &[])?;
    let malloc = instance.get_typed_func::<T, T, _>(&mut store, "malloc")?;
    let free = instance.get_typed_func::<(T, T), (), _>(&mut store, "free")?;
    let mem = instance.get_memory(&mut store, "memory").unwrap();

    if WASM_VALIDATE {
        let validate = instance.get_typed_func::<(T, T), (), _>(&mut store, "validate")?;
        let start = Instant::now();
        let len = input.len().try_into().unwrap();
        let ptr = malloc.call(&mut store, len)?;
        mem.data_mut(&mut store)[ptr.try_into().unwrap()..][..input.len()].copy_from_slice(&input);
        validate.call(&mut store, (ptr, len))?;
        free.call(&mut store, (ptr, len))?;
        Ok(start.elapsed())
    } else {
        let wat2wasm = instance.get_typed_func::<(T, T), T, _>(&mut store, "wat2wasm")?;
        let wasm_ptr = instance.get_typed_func::<T, T, _>(&mut store, "wasm_ptr")?;
        let wasm_len = instance.get_typed_func::<T, T, _>(&mut store, "wasm_len")?;
        let wasm_free = instance.get_typed_func::<T, (), _>(&mut store, "wasm_free")?;

        let start = Instant::now();
        let len = input.len().try_into().unwrap();
        let ptr = malloc.call(&mut store, len)?;
        mem.data_mut(&mut store)[ptr.try_into().unwrap()..][..input.len()].copy_from_slice(&input);
        let wasm_obj = wat2wasm.call(&mut store, (ptr, len))?;
        free.call(&mut store, (ptr, len))?;

        let ret_ptr = wasm_ptr.call(&mut store, wasm_obj)?;
        let ret_len = wasm_len.call(&mut store, wasm_obj)?;

        // let wasm =
        //     mem.data(&store)[ret_ptr.try_into().unwrap()..][..ret_len.try_into().unwrap()].to_vec();
        wasm_free.call(&mut store, wasm_obj)?;
        Ok(start.elapsed())
    }
}
