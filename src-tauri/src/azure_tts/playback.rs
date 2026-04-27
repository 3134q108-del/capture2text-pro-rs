use std::io::Cursor;
use std::sync::mpsc::{self, Sender};
use std::thread;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

#[derive(Clone)]
pub struct PlaybackController {
    tx: Sender<PlaybackCommand>,
}

enum PlaybackCommand {
    Play(Vec<u8>),
    Stop,
}

impl PlaybackController {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<PlaybackCommand>();
        thread::Builder::new()
            .name("azure-tts-playback".to_string())
            .spawn(move || {
                let mut player: Option<Player> = None;
                while let Ok(command) = rx.recv() {
                    match command {
                        PlaybackCommand::Play(bytes) => {
                            if player.is_none() {
                                match Player::new() {
                                    Ok(new_player) => player = Some(new_player),
                                    Err(err) => {
                                        eprintln!("[tts] playback init failed: {err}");
                                        continue;
                                    }
                                }
                            }
                            if let Some(active_player) = player.as_mut() {
                                if let Err(err) = active_player.play(bytes) {
                                    eprintln!("[tts] playback failed: {err}");
                                }
                            }
                        }
                        PlaybackCommand::Stop => {
                            if let Some(active_player) = player.as_mut() {
                                active_player.stop();
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
            .send(PlaybackCommand::Play(mp3_bytes))
            .map_err(|err| err.to_string())
    }

    pub fn stop(&self) {
        let _ = self.tx.send(PlaybackCommand::Stop);
    }
}

struct Player {
    sink: Option<Sink>,
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
}

impl Player {
    fn new() -> Result<Self, String> {
        let (stream, stream_handle) =
            OutputStream::try_default().map_err(|err| err.to_string())?;
        Ok(Self {
            sink: None,
            _stream: stream,
            stream_handle,
        })
    }

    fn play(&mut self, mp3_bytes: Vec<u8>) -> Result<(), String> {
        self.stop();
        if mp3_bytes.is_empty() {
            return Err("empty audio bytes".to_string());
        }
        let source = Decoder::new(Cursor::new(mp3_bytes))
            .map_err(|err| format!("decode mp3 failed: {err}"))?;
        let sink =
            Sink::try_new(&self.stream_handle).map_err(|err| err.to_string())?;
        sink.append(source);
        self.sink = Some(sink);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
    }
}
