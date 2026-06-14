use serde::Serialize;
use thiserror::Error;

#[allow(dead_code)]
#[derive(Debug, Error, Serialize)]
pub enum CommandError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Not found")]
    NotFound,
}
