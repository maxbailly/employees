use std::error;
use std::fmt::{Display, Formatter, Result};

#[derive(Debug)]
pub enum Error {
    ContextArgumentMissing(&'static str),
    StartingThread(std::io::Error),
    Other(Box<dyn error::Error>),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::ContextArgumentMissing(arg_name) => {
                write!(f, "Missing argument {arg_name} in context")
            }
            Self::StartingThread(err) => write!(f, "Failed to run worker thread: {err}"),
            Self::Other(err) => write!(f, "Failed launch worker: {err}"),
        }
    }
}

impl error::Error for Error {}
