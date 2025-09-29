use std::{path::{Path, PathBuf}};

use clap::Parser;

mod main_err;
mod program_info;

mod cmterm;

mod server;
mod repo_management;
mod mission_codes;

use main_err::MainErr;
use program_info::{ProgramArgs, ProgramInfo};

fn repo_path_valid(path: &String) -> Result<(), MainErr> {
    if path.to_lowercase().as_str().eq("exit") { return Ok(()); }

    let repo_path = Path::new(path);
    match repo_path.exists() {
        true => (),
        false => return Err("inputted path does not exist on filesystem".into())
    }

    match repo_management::get_repo(path) {
        Ok(_) => (),
        Err(e) => return Err(e.into())
    }

    return Ok(())
}

fn prompt_repo_path(log: &cmterm::Log) -> Result<Option<PathBuf>, MainErr> {
    let mut v = None;
    loop {
        let repo_path_str = match log.request_string("[\"Exit\" To Cancel] Enter Gist Repo Path // ") {
            Ok(s) => s,
            Err(e) => {
                log.log_err(e.to_string());
                continue;
            }
        };

        if repo_path_str.to_lowercase().eq("exit") {
            break;
        }

        match repo_path_valid(&repo_path_str) {
            Ok(_) => {
                v = Some(PathBuf::from(repo_path_str));
                break;
            },
            Err(e) => {
                log.log_err(e.to_string());
                continue;
            },
        };
    }
    return Ok(v);
}

fn validate_args(mut args: ProgramArgs, log: &cmterm::Log) -> Result<ProgramArgs, MainErr> {
    if args.repo_path.is_some() { return Ok(args); }

    match args.no_interactivity {
        true => {
            args.repo_path = Some(std::env::current_dir()?);
            log.log(format!("No repo path provided, assuming repo to be at\n{}", args.repo_path.as_ref().unwrap().display()));
        },

        false => {
            log.log("No repo path provided, prompting for one...");
            args.repo_path = prompt_repo_path(log)?;
            if args.repo_path.as_ref().is_some() {
                log.log_success(format!("Received valid repo path @ \"{}\"", args.repo_path.as_ref().unwrap().to_string_lossy()));
            }
        }
    };

    return Ok(args);
}

fn run_server(program: &ProgramInfo) -> Result<(), MainErr> {
    // Start Server
    program.main_log.log(format!("Starting server @ localhost:{}", program.args.port));

    let (join, send) = server::start(&program)?;

    if program.args.no_interactivity {
        program.main_log.log("Will yield indefinitely while server runs in non-interactive mode\nQuit with CTRL+C");

        // Should infinitely yield @ join.join()
        return match join.join() {
            Ok(_) => Err("server thread erroneously exited without error in non-interactive mode".into()),
            Err(e) => Err(format!("server thread exited with error {:?} in non-interactive mode", e).into())
        }
    }

    // Wait for enter then kill server
    program.main_log.wait_for_enter("Press ENTER To Kill Server")?;
    match send.send(()) {
        Ok(_) => (),
        Err(e) => return Err(format!("failed to send server kill message with error {e}").into())
    };

    return match join.join() {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Server thread exited with error {:?}!", e).into())
    };
}

fn program_loop(mut program: ProgramInfo) -> Result<(), MainErr> {
    // is_none == true when input.to_lowercase() == "exit"
    if program.args.repo_path.is_none() {
        program.main_log.log("Exiting...");
        return Ok(());
    }

    let repo_path = program.args.repo_path.as_ref().unwrap();

    let _repo = match repo_management::get_repo(repo_path) {
        Ok(r) => r,
        Err(e) => {
            program.main_log.log_warn(
                format!("Failed to initialize repo @ \"{}\" with error:\n{}", repo_path.display(), e)
            );
            program.args.repo_path = prompt_repo_path(&program.main_log)?;
            return program_loop(program);
        }
    };

    match run_server(&program) {
        Ok(_) => (),
        Err(e) => {
            program.main_log.log_err(format!("Server failed to start with error:\n{}", e));
            return Err(e)
        }
    }

    program.main_log.log("Killed Previous Server\nTo open a server for a different repo, please enter a repo path");
    program.args.repo_path = prompt_repo_path(&program.main_log)?;
    return program_loop(program);
}

fn main() {
    let mut args = ProgramArgs::parse();

    let term_man = cmterm::Manager::new();
    let main_log = term_man.main_log.clone();
    let server_log = term_man.server_log.clone();

    let (kill_render, join_renderthread) = term_man.spawn_threads(args.terminal_redraw_delay);

    args = match validate_args(args, &main_log) {
        Ok(a) => a,
        Err(e) => {
            main_log.log_warn(format!("Failed to validate program arguments with\n{}", e));
            return;
        }
    };

    let program = ProgramInfo {
        args: args,
        main_log: main_log,
        srvr_log: server_log
    };

    match program_loop(program) {
        Ok(_) => (),
        Err(e) => println!("Server loop terminated early with error: {}", e)
    }

    kill_render.send(()).unwrap();
    join_renderthread.join().unwrap();
}