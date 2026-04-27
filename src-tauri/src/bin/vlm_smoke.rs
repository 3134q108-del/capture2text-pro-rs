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
            "usage: cargo run --bin vlm_smoke -- <png_path> <zh-TW|zh-CN|en-US|ja-JP|ko-KR|de-DE|fr-FR>",
        ));
    }

    let png_path = &args[1];
    let target_lang = match args[2].as_str() {
        "zh-TW" | "zh" => TargetLang::TraditionalChinese,
        "zh-CN" => TargetLang::SimplifiedChinese,
        "en-US" | "en" => TargetLang::English,
        "ja-JP" | "ja" => TargetLang::Japanese,
        "ko-KR" | "ko" => TargetLang::Korean,
        "de-DE" | "de" => TargetLang::German,
        "fr-FR" | "fr" => TargetLang::French,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "language must be zh-TW / zh-CN / en-US / ja-JP / ko-KR / de-DE / fr-FR",
            ));
        }
    };

    let png_bytes = fs::read(png_path)?;
    let output = ocr_and_translate(&png_bytes, target_lang)
        .map_err(|err| io::Error::other(err.to_string()))?;
    let output_json = serde_json::to_string_pretty(&output)
        .map_err(|err| io::Error::other(format!("serialize output failed: {err}")))?;

    println!("{output_json}");
    Ok(())
}
