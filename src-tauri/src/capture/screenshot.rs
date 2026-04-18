use std::sync::mpsc::Receiver;

use crate::capture::pipeline;
use crate::capture::HotkeyEvent;
use crate::overlay;

pub(crate) fn worker_loop(rx: Receiver<HotkeyEvent>) {
    for event in rx {
        match pipeline::run_for_event(event) {
            Ok(Some(rect)) => {
                println!(
                    "[pipeline] mode={} detected screen=({},{},{},{})",
                    pipeline::mode_label(event.kind),
                    rect.x,
                    rect.y,
                    rect.w,
                    rect.h
                );
                overlay::show(rect);
            }
            Ok(None) => {
                println!("[pipeline] no text block");
            }
            Err(err) => {
                eprintln!("[pipeline] failed: {err}");
            }
        }
    }
}
