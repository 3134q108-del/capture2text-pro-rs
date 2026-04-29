use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

use crate::capture::pipeline;
use crate::capture::{self, CaptureRequest};
use crate::drag_overlay;
use crate::mouse_hook::{self, MouseEvent};
use crate::overlay;
use crate::output_lang;
use crate::vlm::{self, TargetLang};

const WORKER_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(crate) fn worker_loop(rx: Receiver<CaptureRequest>) {
    loop {
        drain_mouse_events();

        let mut request = match rx.recv_timeout(WORKER_POLL_INTERVAL) {
            Ok(request) => request,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };

        loop {
            match rx.try_recv() {
                Ok(newer) => {
                    request = newer;
                    if matches!(request, CaptureRequest::Exit) {
                        break;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }

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

            vlm::try_submit_ocr(
                outcome.png_bytes,
                current_target_lang(),
                pipeline::mode_label(outcome.mode),
            );
        }
        Ok(None) => {
            println!("[pipeline] no text block");
        }
        Err(err) => {
            eprintln!("[pipeline] failed: {err}");
        }
    }
}

fn current_target_lang() -> TargetLang {
    match output_lang::current().as_str() {
        "zh-CN" => TargetLang::SimplifiedChinese,
        "en-US" => TargetLang::English,
        "ja-JP" => TargetLang::Japanese,
        "ko-KR" => TargetLang::Korean,
        "de-DE" => TargetLang::German,
        "fr-FR" => TargetLang::French,
        _ => TargetLang::TraditionalChinese,
    }
}
