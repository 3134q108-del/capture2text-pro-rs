use std::sync::mpsc::Receiver;

use crate::capture::pipeline;
use crate::capture::CaptureRequest;
use crate::overlay;

pub(crate) fn worker_loop(rx: Receiver<CaptureRequest>) {
    for request in rx {
        let mode = pipeline::request_label(request);
        match pipeline::run_for_request(request) {
            Ok(Some(rect)) => {
                println!(
                    "[pipeline] mode={} detected screen=({},{},{},{})",
                    mode,
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
