use std::{error::Error, fmt::{self, Display}, io};

use crate::repo_management::RepoError;
use crate::server::ServerError;

#[derive(Debug)]
pub enum MainErr {
    Generic(String),
    IO(io::Error),
    Repo(RepoError),
    Server(ServerError)
}

impl Display for MainErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MainErr::Generic(s) => f.write_str(&s),
            MainErr::IO(e) => f.write_fmt(format_args!("i/o error: {e}")),
            MainErr::Repo(e) => f.write_fmt(format_args!("repo error: {e}")),
            MainErr::Server(e) => f.write_fmt(format_args!("server error: {e}"))
        }
    }
}

impl Error for MainErr {}

impl From<io::Error> for MainErr {
    fn from(value: io::Error) -> Self {
        return Self::IO(value);
    }
}

impl From<RepoError> for MainErr {
    fn from(value: RepoError) -> Self {
        return Self::Repo(value);
    }
}

impl From<ServerError> for MainErr {
    fn from(value: ServerError) -> Self {
        return Self::Server(value);
    }
}

impl From<&str> for MainErr {
    fn from(value: &str) -> Self {
        return Self::Generic(String::from(value));
    }
}

impl From<String> for MainErr {
    fn from(value: String) -> Self {
        return Self::Generic(value);
    }
}