use std::env;
use std::fs;
use std::io;

use capture2text_pro_rs_lib::vlm::ocr_and_translate;

fn main() {
    if let Err(err) = run() {
        eprintln!("vlm_smoke failed: {err}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: cargo run --bin vlm_smoke -- <png_path> <native_lang> <target_lang>",
        ));
    }

    let png_path = &args[1];
    let native_lang = normalize_lang_arg(&args[2])?;
    let target_lang = normalize_lang_arg(&args[3])?;
    if native_lang == target_lang {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native_lang and target_lang must be different",
        ));
    }

    let png_bytes = fs::read(png_path)?;
    let output = ocr_and_translate(&png_bytes, &native_lang, &target_lang)
        .map_err(|err| io::Error::other(err.to_string()))?;
    let output_json = serde_json::to_string_pretty(&output)
        .map_err(|err| io::Error::other(format!("serialize output failed: {err}")))?;

    println!("{output_json}");
    Ok(())
}

fn normalize_lang_arg(raw: &str) -> io::Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "language code must not be empty",
        ));
    }

    let canonical_guess = match trimmed.to_ascii_lowercase().as_str() {
        "zh" | "zh-tw" => "zh-TW",
        "zh-cn" => "zh-CN",
        "en" | "en-us" => "en-US",
        "ja" | "ja-jp" => "ja-JP",
        "ko" | "ko-kr" => "ko-KR",
        "fr" | "fr-fr" => "fr-FR",
        "de" | "de-de" => "de-DE",
        _ => trimmed,
    };

    const SUPPORTED_CODES: &[&str] = &[
        "zh-CN", "zh-TW", "en-US", "ja-JP", "ko-KR", "fr-FR", "de-DE", "es-ES", "pt-PT", "it-IT",
        "ru-RU", "vi-VN", "ar-SA", "id-ID", "th-TH", "hi-IN", "el-GR", "he-IL", "tr-TR", "pl-PL",
        "nl-NL", "uk-UA", "cs-CZ", "sv-SE", "da-DK", "no-NO", "fi-FI", "hu-HU", "ro-RO", "bg-BG",
        "ms-MY", "fil-PH",
    ];

    if let Some(code) = SUPPORTED_CODES
        .iter()
        .find(|code| code.eq_ignore_ascii_case(canonical_guess))
    {
        return Ok((*code).to_string());
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("unsupported language code: {raw}"),
    ))
}
