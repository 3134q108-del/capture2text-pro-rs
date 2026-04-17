# Stage 2b Build Setup (Leptonica via vcpkg)

This document describes the prerequisite build environment for Leptonica-based image processing in `src-tauri`.

## Required tools

- Windows 10/11
- Rust (MSVC toolchain)
- vcpkg at `C:\vcpkg`
- LLVM (for `libclang.dll`, used by `bindgen`)

## vcpkg setup

```powershell
cd C:\vcpkg
.\bootstrap-vcpkg.bat
.\vcpkg integrate install
.\vcpkg install leptonica:x64-windows-static-md
[Environment]::SetEnvironmentVariable('VCPKG_ROOT', 'C:\vcpkg', 'User')
```

## Why `x64-windows-static-md`

- Rust/Tauri MSVC builds use dynamic CRT (`/MD`) by default.
- `x64-windows-static-md` gives static Leptonica libraries with dynamic CRT linkage.
- This avoids CRT mismatch issues (for example `LNK2038`) that happen with `/MT` triplets.

## libclang for bindgen

Install LLVM and set:

```powershell
[Environment]::SetEnvironmentVariable('LIBCLANG_PATH', 'C:\Program Files\LLVM\bin', 'User')
```

`libclang.dll` must be discoverable by `bindgen` during `cargo` build.

## Rust project settings

- `src-tauri/Cargo.toml` includes `leptonica-sys = "0.4.9"`.
- `src-tauri/Cargo.toml` sets `default-run = "capture2text-pro-rs"` so `cargo run` / `tauri dev` pick the main app unambiguously (not the `leptonica_check` smoke-test bin).
- `.cargo/config.toml` (at repo root, discovered by cargo walking up from `src-tauri/`) sets:

```toml
[env]
VCPKGRS_TRIPLET = "x64-windows-static-md"
```

## Wrapper structure

- `src-tauri/src/leptonica/mod.rs` — safe `Pix` wrapper (library module, exposed via `lib.rs` as `pub mod leptonica`). Consumed internally by `capture2text_pro_rs_lib::leptonica::{Pix, LeptonicaError}`.
- `src-tauri/src/bin/leptonica_check.rs` — smoke-test binary.

Rationale: keeping the wrapper out of `src/bin/` prevents cargo auto-discovery from treating it as a binary target (which would make `cargo run` ambiguous and break `tauri dev`).

## Unicode paths (important)

Leptonica's `pixRead` / `pixWrite` use C `fopen`, which on Windows is ANSI-codepage only and **fails** on paths containing non-ASCII characters (e.g. the Chinese segments in this repo's own path, or a user's Chinese Windows profile name).

The wrapper bypasses this entirely by using memory-based I/O:

- `Pix::read(path)` reads file bytes via `std::fs::read`, then calls `pixReadMem`.
- `Pix::write_png(path)` serialises via `pixWriteMem`, frees the Leptonica-owned buffer with `lept_free`, then writes via `std::fs::write`.

All file-system path handling is done by Rust; Leptonica never sees a filename. This is also the pattern the production OCR pipeline will use (bytes in/out, no disk round-trip per frame).

## Smoke test binary

Run:

```powershell
cd src-tauri
cargo run --bin leptonica_check
```

Pipeline executed:

```
read (any depth) → convert_to_8 → scale(1.25, 1.25) → convert_to_1(threshold=128)
  → remove_border_conn_comps(conn=8) → conn_comp_count(conn=8) → write_png
```

Behavior:

- Input image: latest PNG in `%LOCALAPPDATA%\Capture2TextPro\captures\`
- Output image: `src-tauri/target/leptonica_check/leptonica_check_<timestamp>.png` (1bpp binary, scaled 1.25x)
- Verifies write/read round-trip by re-loading the output via `pixReadMem` and asserting dimensions + 1bpp depth.

The output path is intentionally inside workspace `target/` so smoke tests remain sandbox/CI-friendly and do not require write access outside the repo. This does not change production app output behavior.
