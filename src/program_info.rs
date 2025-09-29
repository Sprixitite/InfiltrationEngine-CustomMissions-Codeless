use std::{path::PathBuf, sync::Arc};
use clap::{arg, command, Parser};

use crate::cmterm;

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
pub struct ProgramArgs {
    /// The path to the custom mission gist repo on disk, defaults to current working directory
    #[arg(short='r', long="repo-path")]
    pub repo_path: Option<PathBuf>,

    /// The local port on which to start the internal http server
    #[arg(short='p', long="port", default_value_t=47362)]
    pub port: u16,

    /// The delay between passive terminal redraws in milliseconds
    /// Does not affect redraws which occur when requesting/receiving user input in interactive mode
    #[arg(short='d', long="redraw-delay", default_value_t=250)]
    pub terminal_redraw_delay: u64,

    /// Disables interactive terminal interface
    #[arg(short='e', long="no-interact", default_value_t=false)]
    pub no_interactivity: bool,

    #[arg(short='c', long="hide-url", default_value_t=false)]
    pub hide_url: bool,
}

pub struct ProgramInfo {
    pub args: ProgramArgs,
    pub main_log: Arc<cmterm::Log>,
    pub srvr_log: Arc<cmterm::Log>
}