use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use crate::capture::pipeline;
use crate::capture::{self, CaptureRequest};
use crate::drag_overlay;
use crate::mouse_hook::{self, MouseEvent};
use crate::overlay;
use crate::vlm::{self, TargetLang};

const WORKER_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(crate) fn worker_loop(rx: Receiver<CaptureRequest>) {
    loop {
        drain_mouse_events();

        let request = match rx.recv_timeout(WORKER_POLL_INTERVAL) {
            Ok(request) => request,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };

        if matches!(request, CaptureRequest::Exit) {
            break;
        }

        process_request(request);
    }
}

fn drain_mouse_events() {
    while let Some(event) = mouse_hook::try_recv_event() {
        match event {
            MouseEvent::LeftDown => {
                let rect = drag_overlay::finalize_and_get_rect();
                if let Some(rect) = rect {
                    capture::try_enqueue_request(CaptureRequest::SelectedRect {
                        rect,
                        queued_at: Instant::now(),
                    });
                }
            }
            MouseEvent::RightDown | MouseEvent::RightUp => {}
        }
    }
}

fn process_request(request: CaptureRequest) {
    match pipeline::run_for_request(request) {
        Ok(Some(outcome)) => {
            println!(
                "[pipeline] mode={} detected screen=({},{},{},{})",
                pipeline::mode_label(outcome.mode),
                outcome.rect.x,
                outcome.rect.y,
                outcome.rect.w,
                outcome.rect.h
            );
            overlay::show(outcome.rect);

            let png = outcome.png_bytes;
            thread::spawn(move || match vlm::ocr_and_translate(&png, TargetLang::Chinese) {
                Ok(out) => {
                    println!("[vlm] original: {}", out.original);
                    println!("[vlm] translated: {}", out.translated);
                    println!("[vlm] duration_ms: {}", out.duration_ms);
                }
                Err(err) => eprintln!("[vlm] failed: {err}"),
            });
        }
        Ok(None) => {
            println!("[pipeline] no text block");
        }
        Err(err) => {
            eprintln!("[pipeline] failed: {err}");
        }
    }
}
