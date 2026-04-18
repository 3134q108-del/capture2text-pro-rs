use std::env;
use std::fs;
use std::io;

use capture2text_pro_rs_lib::vlm::{ocr_and_translate, TargetLang};

fn main() {
    if let Err(err) = run() {
        eprintln!("vlm_smoke failed: {err}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: cargo run --bin vlm_smoke -- <png_path> <zh|en>",
        ));
    }

    let png_path = &args[1];
    let target_lang = match args[2].as_str() {
        "zh" => TargetLang::Chinese,
        "en" => TargetLang::English,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "language must be zh or en",
            ));
        }
    };

    let png_bytes = fs::read(png_path)?;
    let output = ocr_and_translate(&png_bytes, target_lang)?;
    let output_json = serde_json::to_string_pretty(&output)
        .map_err(|err| io::Error::other(format!("serialize output failed: {err}")))?;

    println!("{output_json}");
    Ok(())
}
