use std::sync::Mutex;
use std::{error::Error, fmt::Display, io::Read, sync::mpsc::Sender, thread::JoinHandle};

use git2::Repository;
use rouille::Request;
use rouille::{Response, ResponseBody};

use crate::program_info::{self, ProgramInfo};

use crate::{cmterm, repo_management};
use crate::mission_codes;

#[derive(Debug)]
pub enum ServerError {
    RepoErr(repo_management::RepoError),

    // Thank you rouille for this amazing type signature
    RouilleErr(Box<dyn Error + Send + Sync + 'static>)
}

impl Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerError::RepoErr(e) => e.fmt(f),
            ServerError::RouilleErr(e) => e.fmt(f),
        }
    }
}

impl Error for ServerError {}

impl From<repo_management::RepoError> for ServerError {
    fn from(value: repo_management::RepoError) -> Self {
        return ServerError::RepoErr(value);
    }
}

fn server_error(msg: impl Into<String>) -> Response {
    return Response { 
        status_code: 200,
        headers: vec![],
        data: ResponseBody::from_string(msg),
        upgrade: None
    }
}

fn server_requests_loop(request: &Request, repo: &Repository, log: &cmterm::Log) -> Response {
    let requrl = request.url();
    let reqmethod = request.method();

    if requrl != "/publish_codeless" {
        log.log_err(format!("Received request to invalid endpoint \'{}\'", requrl));
        return Response::empty_400();
    }
    if reqmethod != "POST" {
        log.log_err(format!("Received request to /publish_codeless of invalid HTTP method \'{}\'", reqmethod));
        return Response::empty_400();
    }

    let mut reqbody = match request.data() {
        Some(d) => d,
        None => {
            log.log_err("Internal server error whilst attempting to retrieve response body\nThis is a bug, please report it");
            return server_error("error retrieving response body");
        }
    };

    // Allocate 200,000 bytes
    // This is the maximum size of a single code, so I figure it's a reasonable default
    let mut body_read = Vec::<u8>::with_capacity(200_000);
    match reqbody.read_to_end(&mut body_read) {
        Ok(_) => (),
        Err(e) => {
            log.log_err(format!("Error occurred whilst reading request body to internal buffer:\n{}", e));
            return server_error("error reading response body to internal buffer");
        }
    };

    let body_str = match String::from_utf8(body_read) {
        Ok(s) => s,
        Err(e) => {
            log.log_err(format!("Request body was not a valid UTF-8 string, with reason:\n{}", e));
            return server_error("error converting response body to string");
        }
    };

    log.log("Got POST request to /publish_codeless with valid string body");

    let mut mission_code = match mission_codes::MissionCode::parse_from(&body_str) {
        Ok(c) => c,
        Err(e) => return {
            log.log_err(format!("Request body was not a valid custom mission code, with reason:\n{}", e));
            server_error(format!("error \'{e}\' encountered while parsing mission code"))
        }
    };

    let program_args = program_info::get_args();

    let gist_url = match &mission_code.gist_url {
        Some(s) => s.clone(),
        None => repo_management::remote_url_from_name(repo, &mission_code.gist_remote.as_ref().expect("Mission should have remote to be valid")).expect("URL should exist for remote").expect("URL should exist for remote")
    };

    let gist_url_display = match program_args.hide_url {
        true => "*".repeat(gist_url.len()),
        false => gist_url
    };

    log.log_success(
        format!(
            "Parsed sent mission code, details are as follows:\nVersion: {}\nGist File: {}\nGist URL: {}\nGist Remote: {}\nFeature Count: {}\nFeatures: [{}]",
            mission_code.codeless_fmt_version.version(),
            &mission_code.gist_file,
            gist_url_display,
            &mission_code.gist_remote.as_ref().unwrap_or(&String::from("None")),
            mission_code.codeless_features.len(),
            mission_code.feature_display()
        )
    );

    log.log("Attempting to commit to repo...");
    match repo_management::publish(repo, &mut mission_code, None, None) {
        Ok(_) => log.log_success("Success...?"),
        Err(e) => {
            log.log_err(e.to_string());
        }
    }

    return Response::text("Hello, World! Again!");
}

pub fn start(program: &ProgramInfo) -> Result<(JoinHandle<()>, Sender<()>), ServerError> {
    let program_args = program_info::get_args();

    let srvr_log = program.srvr_log.clone();
    let repo = Mutex::new(repo_management::get_repo(program.repo_path.as_ref().expect(""))?);
    let server_start_result = rouille::Server::new(format!("localhost:{}", program_args.port), move | request | {
        cmterm::Log::set(srvr_log.clone());
        let repo = &repo;
        return server_requests_loop(request, &repo.lock().unwrap(), &srvr_log);
    });

    let server = match server_start_result {
        Ok(s) => s,
        Err(e) => return Err(ServerError::RouilleErr(e))
    };

    program.srvr_log.log_success("Started Server");

    return Ok(server.stoppable())
}