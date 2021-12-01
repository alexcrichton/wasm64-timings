use anyhow::Result;
use std::env;
use std::io::Write;
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

    fn diff_pct(baseline: Duration, dur: Duration) -> f64 {
        let (neg, diff) = if dur > baseline {
            (false, dur - baseline)
        } else {
            (true, baseline - dur)
        };
        let pct = (diff.as_nanos() as f64 / baseline.as_nanos() as f64) * 100.0;
        if neg {
            -pct
        } else {
            pct
        }
    }

    // run the 32-bit wasm with default wasmtime settings (aka no bounds checks)
    let dur32 = run::<u32>(&Config::new(), wasm32, &input)?;
    println!("baseline wasm32: {:.02?}", dur32);

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
    println!(
        "         native: {:.02}% ({:.02?})",
        diff_pct(dur32, native_dur),
        native_dur,
    );

    const WIDTH: usize = 15;

    let configs = configs();

    let print_header = || {
        print!("|        |");
        for (name, _) in configs.iter() {
            print!(" {:1$} |", name, WIDTH);
        }
        println!();
        print!("|--------|");
        for _ in 0..configs.len() {
            print!("{:-<1$}|", "-", WIDTH + 2);
        }
        println!();
    };

    println!("\n------ bounds checks timings relative to wasm32 baseline ------\n");
    print_header();
    let mut timings = Vec::new();
    print!("| wasm32 |");
    std::io::stdout().flush()?;
    for (_, config) in configs.iter() {
        let dur = run::<u32>(config, wasm32, &input)?;
        print!("{:>+1$.02}% |", diff_pct(dur32, dur), WIDTH);
        std::io::stdout().flush()?;
        timings.push(dur);
    }
    println!();
    print!("| wasm64 |");
    std::io::stdout().flush()?;
    for (_, config) in configs.iter() {
        let dur = run::<u64>(config, wasm64, &input)?;
        print!("{:>+1$.02}% |", diff_pct(dur32, dur), WIDTH);
        std::io::stdout().flush()?;
        timings.push(dur);
    }
    println!();
    println!();
    println!();

    println!("\n------ bounds checks timings ------\n");
    print_header();
    let mut timings = timings.iter();
    print!("| wasm32 |");
    for _ in 0..configs.len() {
        let dur = timings.next().unwrap();
        print!("{:>1$} |", format!("{:.02?}", dur), WIDTH + 1);
    }
    println!();
    print!("| wasm64 |");
    for _ in 0..configs.len() {
        let dur = timings.next().unwrap();
        print!("{:>1$} |", format!("{:.02?}", dur), WIDTH + 1);
    }
    println!();

    Ok(())
}

fn configs() -> Vec<(&'static str, Config)> {
    let mut configs = Vec::new();
    let mut base = Config::new();
    base.wasm_memory64(true);
    unsafe {
        configs.push((
            "static",
            base.clone()
                .static_memory_maximum_size(2 << 30)
                .static_memory_forced(true)
                .cranelift_flag_set("enable_heap_access_spectre_mitigation", "false")
                .unwrap()
                .clone(),
        ));
        configs.push((
            "dynamic",
            base.clone()
                .static_memory_maximum_size(0)
                .cranelift_flag_set("enable_heap_access_spectre_mitigation", "false")
                .unwrap()
                .clone(),
        ));
        configs.push((
            "static-spectre",
            base.clone()
                .static_memory_maximum_size(2 << 30)
                .static_memory_forced(true)
                .clone(),
        ));
        configs.push((
            "dynamic-spectre",
            base.clone().static_memory_maximum_size(0).clone(),
        ));
    }
    configs
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

        let wasm =
            mem.data(&store)[ret_ptr.try_into().unwrap()..][..ret_len.try_into().unwrap()].to_vec();
        wasm_free.call(&mut store, wasm_obj)?;
        drop(wasm); // TODO: verify against native
        Ok(start.elapsed())
    }
}
