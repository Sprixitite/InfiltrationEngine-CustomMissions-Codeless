use core::time;
use std::{error::Error, ffi::OsStr, fmt::Display, io::{self, Read, Write}, path::Path, process::{Command, ExitStatus}, env, process};

use arboard::Clipboard;

#[cfg(target_os="linux")]
use arboard::SetExtLinux;

// Identifier appended to the beginning of custom missions codes supported by this tool
// Followed by settings, then code contents verbatim
const CODELESS_CM_IDENTIFIER: &'static str = "_infilengine_cm_codeless";

const MISSION_VERSION_FILE: &'static str = ".custommissionversion";

#[derive(Debug)]
struct CodelessMissionData {
    pub gist_url: String,
    pub gist_file: String,
    pub std_code_contents: String,
    pub repo_mission_version: u32,
    pub codeless_fmt_version: u32,
}

#[cfg(target_os="linux")]
const DAEMON_ARG: &'static str = "--daemonize";

#[cfg(target_os="linux")]
fn handle_clipboard_daemon() -> bool {
    match env::args().nth(1).as_deref() == Some(DAEMON_ARG) {
        false => return false,
        true => ()
    };

    let clip_str = match env::args().nth(2) {
        Some(s) => s,
        None => {
            println!("{0} specified but no string was given? Running as if {0} was never passed.", DAEMON_ARG);
            return false;
        }
    };

    let mut clipboard = match get_clipboard() {
        Ok(c) => c,
        Err(e) => {
            println!("Attempting to daemonize but encountered error \"{}\"!", e);
            return false;
        }
    };

    match clipboard.set().wait().text(clip_str) {
        Ok(_) => (),
        Err(e) => println!("Attempting to daemonize but encountered error \"{}\"!", e)
    }
    
    return true;
}

#[cfg(not(target_os="linux"))]
fn handle_clipboard_daemon() -> bool { return false }

fn main() {
    if handle_clipboard_daemon() { return; }

    let mut standby_mode = false;
    for arg in env::args() {
        standby_mode = arg == "--standby";
        if standby_mode { break; }
    }

    println!("Checking git status...");
    match check_git_status() {
        Ok(_) => (),
        Err(e) => return println!("Encountered error {} while checking git state", e)
    };
    
    println!("Acquiring handle to clipboard...");
    let mut clipboard = match get_clipboard() {
        Ok(c) => c,
        Err(e) => return println!("Encountered error {} when attempting to retrieve clipboard!", e)
    };

    let (url, cm_info) = match standby_mode {
        true => standby_loop(&mut clipboard),
        false => {
            match push_from_clipboard(&mut clipboard) {
                Ok(s) => s,
                Err(e) => return println!("{}", e)
            }
        }
    };
    
    println!("Copying gist code to clipboard...");
    match set_clipboard_text(&mut clipboard, &url) {
        Ok(_) => (),
        Err(e) => return println!("{}", e)
    };

    println!("Successfully pushed mission version {}!", cm_info.repo_mission_version);

}

fn standby_loop(clipboard: &mut Clipboard) -> ! {
    println!("Program is running in standby mode! Program may be killed by pressing CTRL+C");

    let mut is_first_loop = true;
    let mut last_clipboard = String::from("");
    loop {
        if !is_first_loop {
            // Wait for half a second to avoid repeatedly querying the clipboard
            std::thread::sleep(time::Duration::from_millis(500));
        }
        is_first_loop = false;

        let current_clipboard = match clipboard.get_text() {
            Ok(s) => s,
            Err(e) => {
                println!("Failed to retrieve clipboard text with error {}!", e);
                continue;
            }
        };

        match current_clipboard == last_clipboard {
            true => continue,
            false => ()
        };

        let (url, cm_info) = match push_from_code(&current_clipboard) {
            Ok(s) => s,
            Err(e) => match e {
                GitError::CodeNotMission => continue,
                _ => {
                    println!("Encountered error {} while attempting to push mission from clipboard!", e);
                    continue;
                }
            }
        };

        match set_clipboard_text(clipboard, &url) {
            Ok(_) => (),
            Err(e) => {
                println!("Encountered error {} while attempting to set clipboard text!", e);
                continue;
            }
        }

        last_clipboard = current_clipboard;
        println!("Successfully pushed mission version {}!", cm_info.repo_mission_version)
    }

}

fn push_from_clipboard(clipboard: &mut Clipboard) -> Result<(String, CodelessMissionData), GitError> {
    return push_from_code(&get_clipboard_text(clipboard)?);
}

fn push_from_code(code: &str) -> Result<(String, CodelessMissionData), GitError> {
    let mut cm_info = match parse_mission_data(code) {
        Ok(i) => i,
        Err(e) => return Err(e)
    };

    println!("Running relevant git operations...");
    cm_info.repo_mission_version += 1;
    return push_codeless_mission(&cm_info).map(|url| {(url, cm_info)});
}

#[cfg(target_os="linux")]
fn set_clipboard_text(_clipboard: &mut Clipboard, to: &str) -> Result<(), GitError> {
    let self_path = match env::current_exe() {
        Ok(p) => p,
        Err(e) => return Err(GitError::IoFailed(String::from("get program path"), e))
    };

    // Spawn clone of this program with daemon argument
    let res = process::Command::new(self_path)
            .arg(DAEMON_ARG)
            .arg(to)
            .stdin(process::Stdio::null())
            .stdout(process::Stdio::null())
            .stderr(process::Stdio::null())
            .current_dir("/")
            .spawn();
    
    return match res {
        Ok(_) => Ok(()),
        Err(e) => Err(GitError::IoFailed(String::from("spawn daemon"), e))
    }
}

#[cfg(not(target_os="linux"))]
fn set_clipboard_text(clipboard: &mut Clipboard, to: &str) -> Result<(), GitError> {
    return match clipboard.set_text(to) {
        Ok(_) => Ok(()),
        Err(e) => Err(GitError::ClipboardFailed(String::from("set clipboard text"), e))
    };
}

fn get_clipboard_text(clipboard: &mut Clipboard) -> Result<String, GitError> {
    return match clipboard.get_text() {
        Ok(s) => Ok(s),
        Err(e) => Err( GitError::ClipboardFailed(String::from("get clipboard text"), e) )
    }
}

fn parse_mission_data(code: &str) -> Result<CodelessMissionData, GitError> {
    if !code.starts_with(CODELESS_CM_IDENTIFIER) { return Err(GitError::CodeNotMission) }

    let list: Vec<&str> = code.splitn(5, "|").collect();

    if list.len() != 5 { return Err(GitError::CodeHeaderTooShort); }

    let mission_version = read_mission_version()?;
    let codeless_fmt_version_str = list.get(3).unwrap();

    return Ok(CodelessMissionData {
        gist_url: list.get(1).unwrap().to_string(),
        gist_file: list.get(2).unwrap().to_string(),
        std_code_contents: list.get(4).unwrap().to_string(),
        repo_mission_version: mission_version.unwrap_or(0),
        codeless_fmt_version: codeless_fmt_version_str.parse().unwrap()
    })
}

#[derive(Debug)]
enum GitError {
    IoFailed(String, io::Error),
    ClipboardFailed(String, arboard::Error),
    CmdOutMangled(String, std::str::Utf8Error),
    WrongRemoteRepo(String, String),
    VersionMalformed(String),

    GitAddFailed,
    GitCommitFailed,
    GitPushFailed,
    GitNotInstalled,
    NotInRepo,
    NoRemoteRepo,
    CodeHeaderTooShort,
    CodeNotMission,
}

impl Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitAddFailed      => f.write_str("git add failed"),
            Self::GitCommitFailed   => f.write_str("git commit failed"),
            Self::GitPushFailed     => f.write_str("git push failed"),
            Self::GitNotInstalled   => f.write_str("system git not found"),
            Self::NotInRepo         => f.write_str("not in git repo"),
            Self::NoRemoteRepo      => f.write_str("repo has no remote"),
            Self::CodeHeaderTooShort => f.write_str("code header too short"),
            Self::CodeNotMission => f.write_str("clipboard contents are not a custom mission"),
            
            Self::WrongRemoteRepo(e, g)   => f.write_fmt(format_args!("current repo remote ({}) doesn't match that included with the mission code ({})", g, e)),
            Self::IoFailed(c, e) => f.write_fmt(format_args!("i/o operation {} failed with error {}", c, e)),
            Self::ClipboardFailed(c, e) => f.write_fmt(format_args!("clipboard operation {} failed with error {}", c, e)),
            Self::CmdOutMangled(c, e) => f.write_fmt(format_args!("command {} outputted invalid utf8 with {}", c, e)),
            Self::VersionMalformed(v) => f.write_fmt(format_args!("repository version malformed, expected u32 as string, got {}", v))
        }
    }
}

impl Error for GitError {}

fn check_git_status() -> Result<(), GitError> {
    let mut cmd = Command::new("git");
    cmd.args(["rev-parse", "--is-inside-work-tree"]);

    return match cmd.output() {
        Ok(o) => match o.status.success() {
            true    => Ok(()),
            false   => Err(GitError::NotInRepo)
        }
        Err(e) => match e.kind() {
            io::ErrorKind::NotFound => Err(GitError::GitNotInstalled),
            _ => Err(GitError::IoFailed(format!("{:?}", cmd), e))
        }
    };
}

fn git_command_status<I, S>(args: I, on_fail: Option<GitError>) -> Result<ExitStatus, GitError>
    where I : IntoIterator<Item = S> + Clone,
          S : AsRef<OsStr> + std::fmt::Display,
{
    check_git_status()?;

    let mut cmd = Command::new("git");
    cmd.args(args.clone());

    let mut args_str = String::with_capacity(64);
    args_str.push('[');
    for arg in args {
        args_str.push_str(&arg.to_string());
        args_str.push_str(", ");
    }
    args_str.push(']');

    return match cmd.output() {
        Ok(o) => {
            match on_fail {
                Some(ge) => match o.status.success() {
                    true => Ok(o.status),
                    false => {
                        println!("{:?}", o);
                        println!("Encountered error {} while running git command {}", ge, args_str);
                        Err(ge)
                    }
                },
                None => Ok(o.status)
            }
        },
        Err(e) => {
            println!("Encountered error {} while running git command {}", e, args_str);
            Err(GitError::IoFailed(format!("{:?}", cmd), e))
        }
    };
}

fn git_command_stdout<I, S>(args: I, cmd_fail_err: GitError) -> Result<String, GitError> 
    where I : IntoIterator<Item = S> + Clone,
          S : AsRef<OsStr> + std::fmt::Display,
{
    check_git_status()?;

    let mut cmd = Command::new("git");
    cmd.args(args);

    return match cmd.output() {
        Ok(o) => match o.status.success() {
            true => match String::from_utf8(o.stdout) {
                Ok(s) => Ok(s.trim().to_string()),
                Err(e) => Err(GitError::CmdOutMangled(format!("{:?}", cmd), e.utf8_error()))
            },
            false => Err(cmd_fail_err)
        }
        Err(e) => Err(GitError::IoFailed(format!("{:?}", cmd), e))
    };
}

fn get_current_commit_hash() -> Result<String, GitError> {
    return git_command_stdout(["rev-parse", "HEAD"], GitError::NotInRepo);
}

fn get_remote_url() -> Result<String, GitError> {
    return git_command_stdout(["config", "--get", "remote.origin.url"], GitError::NoRemoteRepo);
}

fn push_codeless_mission(data: &CodelessMissionData) -> Result<String, GitError> {
    match get_remote_url() {
        Ok(url) => match url == data.gist_url {
            true => (),
            false => return Err(GitError::WrongRemoteRepo(data.gist_url.clone(), url))
        },
        Err(e) => return Err(e)
    };

    // Gist url should match

    let mut code_file = match std::fs::OpenOptions::new().create(true).truncate(true).write(true).open(&data.gist_file) {
        Ok(f) => f,
        Err(e) => return Err(GitError::IoFailed(format!("open code file {}", data.gist_file), e))
    };

    match code_file.write_all(data.std_code_contents.as_bytes()) {
        Ok(_) => (),
        Err(e) => return Err(GitError::IoFailed(format!("write code file {}", data.gist_file), e))
    }
    drop(code_file); // Close the file

    write_mission_version(data.repo_mission_version)?;

    // Add code + version to commit
    git_command_status(["add", &data.gist_file], Some(GitError::GitAddFailed))?;
    git_command_status(["add", MISSION_VERSION_FILE], Some(GitError::GitAddFailed))?;

    // Commit changes
    git_command_status(["commit", "-m", &format!("v{}", data.repo_mission_version)], Some(GitError::GitCommitFailed))?;

    // Push
    git_command_status(["push"], Some(GitError::GitPushFailed))?;
    
    return Ok(format!(
        "{}/raw/{}/{}",
        data.gist_url.replace("gist.github.com", "gist.githubusercontent.com").trim_end_matches("/"),
        get_current_commit_hash()?,
        data.gist_file
    ))
}

fn read_mission_version() -> Result<Option<u32>, GitError> {
    let mission_version_exists = Path::new(MISSION_VERSION_FILE).exists();
    if !mission_version_exists {
        return Ok(None);
    }
    
    return match std::fs::OpenOptions::new().read(true).open(MISSION_VERSION_FILE) {
        Ok(mut f) => {
            // 10 characters can contain 32 bit unsigned integer limit
            let mut ver_str = String::with_capacity(10);
            match f.read_to_string(&mut ver_str) {
                Ok(_) => (),
                Err(e) => return Err(GitError::IoFailed(String::from("read mission version"), e))
            }

            return match ver_str.parse::<u32>() {
                Ok(u) => Ok(Some(u)),
                Err(_) => Err(GitError::VersionMalformed(ver_str))
            };
        },
        Err(e) => Err(GitError::IoFailed(String::from("open mission version file for read"), e))
    };
}

fn write_mission_version(version: u32) -> Result<(), GitError> {
    return match std::fs::OpenOptions::new().create(true).write(true).truncate(true).open(MISSION_VERSION_FILE) {
        Ok(mut f) => {
            match f.write_all(version.to_string().as_bytes()) {
                Ok(_) => Ok(()),
                Err(e) => Err(GitError::IoFailed(String::from("write mission version file"), e))
            }
        },
        Err(e) => Err(GitError::IoFailed(String::from("open mission version file for write"), e))
    }
}

fn get_clipboard() -> Result<Clipboard, arboard::Error> {
    return get_clipboard_recurse(0);
}

const MAX_CLIPBOARD_GET_ATTEMPTS: i32 = 5;
fn get_clipboard_recurse(times_recursed: i32) -> Result<Clipboard, arboard::Error> {
    let clipboard_res = Clipboard::new();

    match clipboard_res {
        Ok(_) => clipboard_res,
        Err(_) => {
            match times_recursed < MAX_CLIPBOARD_GET_ATTEMPTS {
                true => get_clipboard_recurse(times_recursed+1),
                false => clipboard_res
            }
        }
    }
}