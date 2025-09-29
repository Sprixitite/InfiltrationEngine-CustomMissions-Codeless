use std::{path::PathBuf, sync::{Arc, OnceLock}};
use clap::{arg, command, Parser};

use crate::cmterm;

pub const DAEMON_ARG: &'static str = "linux-clipboard-daemon";

static PROGRAM_ARGS: OnceLock<ProgramArgs> = OnceLock::new();

pub fn set_args(args: ProgramArgs) -> &'static ProgramArgs {
    PROGRAM_ARGS.set(args).unwrap();
    return get_args();
}

pub fn get_args() -> &'static ProgramArgs {
    match PROGRAM_ARGS.get() {
        Some(args) => args,
        None => panic!("Attempted to retrieve program args before they were initialized!")
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
pub struct ProgramArgs {
    /// The path to the custom mission gist repo on disk, defaults to current working directory in non-interactive mode
    #[arg(short='r', long="repo-path", value_name="PATH", value_hint=clap::ValueHint::DirPath)]
    pub repo_path: Option<PathBuf>,

    /// The local port on which to start the internal http server
    #[arg(short='p', long="port", value_name="PORT", default_value_t=47362)]
    pub port: u16,

    /// The delay between passive terminal redraws in milliseconds
    /// Does not affect redraws which occur when requesting/receiving user input in interactive mode
    #[arg(short='d', long="redraw-delay", value_name="MILLISECONDS", default_value_t=250)]
    pub terminal_redraw_delay: u64,

    /// Disables interactive terminal interface
    #[arg(short='e', long="no-interact", default_value_t=false)]
    pub no_interactivity: bool,

    /// If enabled, censors the gist URL in the server log - I added this so I can stop editing my screenshots
    #[arg(short='c', long="hide-url", default_value_t=false)]
    pub hide_url: bool,

    /// Workaround for the clipboard on Linux
    /// When passed the program will do nothing but run in the background providing the passed string for the OS
    /// The program will then close when the clipboard contents are changed
    #[cfg(target_os="linux")]
    #[arg(long=DAEMON_ARG)]
    pub linux_clipboard_daemon: Option<String>
}

pub struct ProgramInfo {
    pub main_log: Arc<cmterm::Log>,
    pub srvr_log: Arc<cmterm::Log>,
    pub repo_path: Option<PathBuf>
}