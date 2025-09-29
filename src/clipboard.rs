use std::{fmt::Display, io};
use arboard::Clipboard;

use super::program_info;

#[derive(Debug)]
pub enum Error {
    Arboard(arboard::Error),
    
    FailedSpawningDaemon(io::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Arboard(e) => f.write_fmt(format_args!("Clipboard error {:?}", e)),
            Error::FailedSpawningDaemon(e) => f.write_fmt(format_args!("failed spawning clipboard daemon with error {:?}", e))
        }
    }
}

impl std::error::Error for Error {}

impl From<arboard::Error> for Error {
    fn from(value: arboard::Error) -> Self {
        return Error::Arboard(value);
    }
}

#[cfg(target_os="linux")]
pub fn run_as_daemon(content: impl AsRef<str>) -> Result<(), Error> {
    use arboard::SetExtLinux;

    return Ok(Clipboard::new()?.set().wait().text(content.as_ref())?);
}

#[cfg(target_os="linux")]
fn copy_text_platform(_clipboard: &mut Clipboard, content: impl AsRef<str>) -> Result<(), Error> {
    use std::{env, process};

    let self_path = match env::current_exe() {
        Ok(p) => p,
        Err(e) => return Err(Error::FailedSpawningDaemon(e))
    };

    let res = process::Command::new(&self_path)
            .arg(format!("--{}", program_info::DAEMON_ARG))
            .arg(content.as_ref())
            .stdin(process::Stdio::null())
            .stdout(process::Stdio::null())
            .stderr(process::Stdio::null())
            .current_dir("/tmp/")
            .spawn();

    return match res {
        Ok(_) => Ok(()),
        Err(e) => Err(Error::FailedSpawningDaemon(e))
    };
}

#[cfg(not(target_os="linux"))]
fn copy_text_platform(clipboard: &mut Clipboard, content: impl AsRef<str>) -> Result<(), Error> {
    return Ok(clipboard.set_text(content.as_ref())?);
}

pub fn set_text(content: impl AsRef<str>) -> Result<(), Error> {
    let mut clipboard = match Clipboard::new() {
        Ok(c) => c,
        Err(e) => return Err(e.into())
    };

    return copy_text_platform(&mut clipboard, content);
}