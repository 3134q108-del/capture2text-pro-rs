use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use tauri::{AppHandle, Emitter};

#[derive(Clone)]
pub struct PlaybackController {
    tx: Sender<PlaybackCommand>,
}

enum PlaybackCommand {
    Play {
        target: Option<String>,
        bytes: Vec<u8>,
    },
    Stop {
        emit_done: bool,
    },
}

impl PlaybackController {
    pub fn new(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel::<PlaybackCommand>();
        thread::Builder::new()
            .name("azure-tts-playback".to_string())
            .spawn(move || {
                let mut player: Option<Player> = None;
                while let Ok(command) = rx.recv() {
                    match command {
                        PlaybackCommand::Play { target, bytes } => {
                            if player.is_none() {
                                match Player::new(app.clone()) {
                                    Ok(new_player) => player = Some(new_player),
                                    Err(err) => {
                                        eprintln!("[tts] playback init failed: {err}");
                                        continue;
                                    }
                                }
                            }
                            if let Some(active_player) = player.as_mut() {
                                if let Err(err) = active_player.play(target, bytes) {
                                    eprintln!("[tts] playback failed: {err}");
                                }
                            }
                        }
                        PlaybackCommand::Stop { emit_done } => {
                            if let Some(active_player) = player.as_mut() {
                                active_player.stop(emit_done);
                            }
                        }
                    }
                }
            })
            .expect("failed to spawn azure tts playback thread");
        Self { tx }
    }

    pub fn play(&self, mp3_bytes: Vec<u8>) -> Result<(), String> {
        self.tx
            .send(PlaybackCommand::Play {
                target: None,
                bytes: mp3_bytes,
            })
            .map_err(|err| err.to_string())
    }

    pub fn play_for_target(&self, target: String, mp3_bytes: Vec<u8>) -> Result<(), String> {
        self.tx
            .send(PlaybackCommand::Play {
                target: Some(target),
                bytes: mp3_bytes,
            })
            .map_err(|err| err.to_string())
    }

    pub fn stop(&self) {
        let _ = self.tx.send(PlaybackCommand::Stop { emit_done: true });
    }

    pub fn stop_silent(&self) {
        let _ = self.tx.send(PlaybackCommand::Stop { emit_done: false });
    }
}

struct Player {
    app: AppHandle,
    sink: Option<Arc<Sink>>,
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    play_id: Arc<AtomicU64>,
}

impl Player {
    fn new(app: AppHandle) -> Result<Self, String> {
        let (stream, stream_handle) =
            OutputStream::try_default().map_err(|err| err.to_string())?;
        Ok(Self {
            app,
            sink: None,
            _stream: stream,
            stream_handle,
            play_id: Arc::new(AtomicU64::new(0)),
        })
    }

    fn play(&mut self, target: Option<String>, mp3_bytes: Vec<u8>) -> Result<(), String> {
        self.stop(false);
        if mp3_bytes.is_empty() {
            return Err("empty audio bytes".to_string());
        }
        let source = Decoder::new(Cursor::new(mp3_bytes))
            .map_err(|err| format!("decode mp3 failed: {err}"))?;
        let sink =
            Sink::try_new(&self.stream_handle).map_err(|err| err.to_string())?;
        let sink = Arc::new(sink);
        sink.append(source);
        let play_id = self.play_id.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(target) = target {
            spawn_done_monitor(
                self.app.clone(),
                sink.clone(),
                self.play_id.clone(),
                play_id,
                target,
            );
        }
        self.sink = Some(sink);
        Ok(())
    }

    fn stop(&mut self, emit_done: bool) {
        if !emit_done {
            let _ = self.play_id.fetch_add(1, Ordering::SeqCst);
        }
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
    }
}

fn spawn_done_monitor(
    app: AppHandle,
    sink: Arc<Sink>,
    active_id: Arc<AtomicU64>,
    play_id: u64,
    target: String,
) {
    thread::Builder::new()
        .name("azure-tts-done-monitor".to_string())
        .spawn(move || {
            loop {
                if active_id.load(Ordering::SeqCst) != play_id {
                    break;
                }
                if sink.empty() {
                    if active_id.load(Ordering::SeqCst) == play_id {
                        let _ = app.emit("tts-done", serde_json::json!({ "target": target }));
                    }
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
        })
        .expect("failed to spawn azure tts done monitor");
}
