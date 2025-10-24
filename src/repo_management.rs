use std::{error::Error, fmt::{Debug, Display}, fs, io::{self, Read, Seek, Write}, path::{Path, PathBuf}};

use git2::{Index, IndexAddOption, Remote, Repository, Signature};

use crate::cmterm::{self, LogHandle};

#[derive(Debug)]
pub enum RepoError {
    GitErr(git2::Error, String),
    FileInvalid{repo: String, file: String, reason: String},
    FailWrite{err: io::Error, repo: String, file: String},
    FailRead{err: io::Error, repo: String, file: String},
    FileWayTooFuckingBig{repo: String, file: String},
    NoWorkdir(String),

    PublishError(String),
    DeriveError(String),

    HeadCheckFailed(String),
    HeadDetached(String),
    HeadNotBranch(String),

    CloneFailed(String)
}

impl Display for RepoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitErr(e, s) => f.write_fmt(format_args!("encountered error code \'{:?}\' of class \'{:?}\' while attempting to {}\n{:?}", e.code(), e.class(), s, e)),
            Self::FailWrite{err, repo, file} => f.write_fmt(format_args!("failed to write to file {file} in repo {repo} with error {err}")),
            Self::FailRead {err, repo, file} => f.write_fmt(format_args!("failed to read from file {file} in repo {repo} with error {err}")),
            Self::FileInvalid{repo, file, reason} => f.write_fmt(format_args!("failed to write to file {file} in repo {repo} with reason \"{reason}\"")),
            Self::FileWayTooFuckingBig { repo, file } => f.write_fmt(format_args!("file {file} in repo {repo} is too big for program address space")),
            Self::NoWorkdir(r) => f.write_fmt(format_args!("failed to retrieve workdir for repo {r}")),

            Self::HeadCheckFailed(r) => f.write_fmt(format_args!("failed to retrieve repo {r}'s HEAD")),
            Self::HeadDetached(r) => f.write_fmt(format_args!("repo {r}'s HEAD is detached, this is unsupported")),
            Self::HeadNotBranch(r) => f.write_fmt(format_args!("repo {r}'s HEAD does not point to a branch")),

            Self::PublishError(s) => f.write_fmt(format_args!("publish error: {s}")),
            Self::DeriveError(s) => f.write_fmt(format_args!("derive error: {s}")),

            Self::CloneFailed(s) => f.write_fmt(format_args!("clone error: {s}"))
        }
    }
}

impl Error for RepoError { }

pub trait RepoItem {
    fn publishable_children(&self) -> Option<Vec<&dyn RepoPublishable>>;
    fn derivable_children(&mut self) -> Option<Vec<&mut dyn RepoDerivable>>;
}

pub trait RepoPublishable : RepoItem {
    fn publish_message(&self) -> String { unimplemented!(); }

    #[allow(unused_variables)] // should only be unused in default implementation
    fn publish_target_remote(&self, repo: &Repository) -> Result<String, RepoError> { unimplemented!(); }

    #[allow(unused_variables)] // should only be unused in default implementation
    fn publish_target_file(&self) -> String { unimplemented!(); }

    fn repo_publish(&self, repo: &Repository) -> Result<(), RepoError>;
    fn repo_valid(&self, repo: &Repository) -> Result<(), RepoError>;
}

pub trait RepoDerivable : RepoItem {
    fn repo_derive(&mut self, repo: &Repository) -> Result<(), RepoError>;
    fn repo_process(&mut self, repo: &Repository) -> Result<(), RepoError>;
}

pub fn get_repo(path: impl AsRef<Path>) -> Result<Repository, RepoError> {
    return match Repository::open(path) {
        Ok(r) => Ok(r),
        Err(e) => Err(RepoError::GitErr(e, String::from("open repository")))
    };
}

fn get_remotes(repo: &'_ Repository) -> Result<Vec<Remote<'_>>, RepoError> {
    let thread_log = cmterm::Log::get();
    let remote_names = get_remote_names(repo)?;

    return Ok(remote_names.iter().filter_map(|remote_name| {
        match repo.find_remote(remote_name) {
            Ok(r) => Some(r),
            Err(e) => {
                thread_log.log_warn(format!("Remote {} has no url? Error:\n{}", remote_name, e));
                None
            }
        }
    }).collect())
}

fn get_remote_names(repo: &Repository) -> Result<Vec<String>, RepoError> {
    let thread_log = cmterm::Log::get();
    let remotes = match repo.remotes() {
        Ok(r) => r,
        Err(e) => return Err(RepoError::GitErr(e, String::from("retrieve remotes")))
    };

    let mut invalid_count = 0_u32;

    let remote_names = remotes.iter().filter_map(|r| {
        match r {
            Some(s) => Some(s.to_string()),
            None => {
                invalid_count += 1;
                None
            }
        }
    }).collect();

    if invalid_count > 0 {
        thread_log.log_warn(format!("Found {} remotes with non UTF-8 names", invalid_count));
    }

    return Ok(remote_names);
}

fn get_remote_urls(repo: &Repository) -> Result<Vec<String>, RepoError> {
    let thread_log = cmterm::Log::get();
    let remotes = get_remotes(repo)?;

    return Ok(remotes.iter().filter_map(|r| {
        match r.url() {
            Some(url) => Some(url.to_string()),
            None => {
                // Remote name should be valid because remotes are retrieved from get_remotes() using their UTF-8 valid names
                thread_log.log_warn(format!("URL for remote {} is not valid UTF-8", r.name().expect("remote name should be valid UTF-8")));
                None
            }
        }
    }).collect());
}

pub fn has_remote(repo: &Repository, remote_name: &str) -> Result<bool, RepoError> {
    let remote_names = get_remote_names(repo)?;

    return Ok(remote_names.iter().any(|remote| {
        remote.eq(remote_name)
    }));
}

pub fn has_remote_url(repo: &Repository, remote_url: &str) -> Result<bool, RepoError> {
    let remote_urls = get_remote_urls(repo)?;

    return Ok(remote_urls.iter().any(|url| {
        url.eq(remote_url)
    }));
}

pub fn remote_name_from_url(repo: &Repository, remote_url: &str) -> Result<Option<String>, RepoError> {
    let thread_log = cmterm::Log::get();
    let remotes = get_remotes(repo)?;

    let remote = remotes.into_iter().find(|remote| {
        match remote.url() {
            Some(url) => url.eq(remote_url),
            None => {
                thread_log.log_warn(format!("URL for remote {} is not valid UTF-8", remote.name().expect("remote name should be valid UTF-8")));
                false
            }
        }
    });

    return Ok(match remote {
        Some(r) => r.name().map(|s| { s.to_string() }),
        None => None
    });
}

pub fn remote_url_from_name(repo: &Repository, remote_name: &str) -> Result<Option<String>, RepoError> {
    let thread_log = cmterm::Log::get();
    let remotes = get_remotes(repo)?;

    let mut i = 0;
    let remote = remotes.into_iter().find(|remote| {
        match remote.name() {
            Some(name) => name.eq(remote_name),
            None => {
                i += 1;
                false
            }
        }
    });

    if i > 0 {
        thread_log.log_warn("One or more remotes has invalid UTF-8 title");
    }

    return Ok(match remote {
        Some(r) => r.url().map(|s| { s.to_string() }),
        None => None
    })
}

pub fn repo_errname(repo: &Repository) -> String {
    let workdir_option = repo.workdir();
    
    let workdir_valid = workdir_option.is_some_and(|w| w.file_name().is_some());
    return match workdir_valid {
        true => workdir_option.unwrap().file_name().unwrap().display().to_string(),
        false => repo.path().display().to_string()
    }
}

pub fn get_repo_file_path(repo: &Repository, file: &str) -> Result<PathBuf, RepoError> {
    let base_path = match repo.workdir() {
        Some(p) => p,
        None => return Err(RepoError::NoWorkdir(repo_errname(repo)))
    };

    let target_path = Path::join(base_path, file);
    return Ok(target_path);
}

pub fn overwrite_file(repo: &Repository, file: &str, contents: &str) -> Result<(), RepoError> {
    let target_path = get_repo_file_path(repo, file)?;
    let target_valid = !target_path.exists() || target_path.is_file();
    
    if !target_valid {
        return Err(RepoError::FileInvalid { repo: repo_errname(repo), file: file.to_string(), reason: String::from("non-file item exists at path") });
    }

    let mut file_handle = match fs::OpenOptions::new().create(true).write(true).truncate(true).open(target_path) {
        Ok(f) => f,
        Err(e) => return Err(RepoError::FailWrite { err: e, repo: repo_errname(repo), file: file.to_string() })
    };

    match file_handle.write_all(contents.as_bytes()) {
        Ok(_) => (),
        Err(e) => return Err(RepoError::FailWrite { err: e, repo: repo_errname(repo), file: file.to_string() })
    };

    return Ok(())
}

pub fn read_file(repo: &Repository, file: &str) -> Result<String, RepoError> {
    let target_path = get_repo_file_path(repo, file)?;
    
    if !target_path.exists() {
        return Err(RepoError::FileInvalid { repo: repo_errname(repo), file: file.to_string(), reason: String::from("file doesn't exist") });
    }

    if !target_path.is_file() {
        return Err(RepoError::FileInvalid { repo: repo_errname(repo), file: file.to_string(), reason: String::from("non-file item exists at path") })
    }

    let mut file_handle = match fs::OpenOptions::new().read(true).open(target_path) {
        Ok(f) => f,
        Err(e) => return Err(RepoError::FailRead { err: e, repo: repo_errname(repo), file: file.to_string() })
    };

    let file_size = match file_handle.seek(io::SeekFrom::End(0)) {
        Ok(s) => {
            if s > usize::MAX as u64 {
                // allocating a string of size s would be larger than the program's allowed address space
                // so this is an error (that nobody will ever trigger)
                return Err(RepoError::FileWayTooFuckingBig { repo: repo_errname(repo), file: file.to_string() })
            }
            s as usize
        },
        Err(e) => return Err(RepoError::FailRead { err: e, repo: repo_errname(repo), file: file.to_string() })
    };

    match file_handle.seek(io::SeekFrom::Start(0)) {
        Ok(_) => (),
        Err(e) => return Err(RepoError::FailRead { err: e, repo: repo_errname(repo), file: file.to_string() })
    };

    let mut contents = String::with_capacity(file_size);
    match file_handle.read_to_string(&mut contents) {
        Ok(_) => (),
        Err(e) => return Err(RepoError::FailRead { err: e, repo: repo_errname(repo), file: file.to_string() })
    };

    return Ok(contents);
}

fn item_derive_recurse(repo: &Repository, item: &mut dyn RepoDerivable) -> Result<(), RepoError> {
    let valid = item.repo_derive(repo);
    let items = item.derivable_children();
    if valid.is_err() || items.is_none() { return valid; }

    for i in items.unwrap() {
        item_derive_recurse(repo, i)?;
    }

    return Ok(());
}

fn item_process_recurse(repo: &Repository, item: &mut dyn RepoDerivable) -> Result<(), RepoError> {
    let valid = item.repo_process(repo);
    let items = item.derivable_children();
    if valid.is_err() || items.is_none() { return valid; }

    for i in items.unwrap() {
        item_process_recurse(repo, i)?;
    }

    return Ok(());
}

fn item_write_changes_recurse(repo: &Repository, item: &dyn RepoPublishable) -> Result<(), RepoError> {
    let valid = item.repo_publish(repo);
    let items = item.publishable_children();
    if valid.is_err() || items.is_none() { return valid; }

    for i in items.unwrap() {
        item_write_changes_recurse(repo, i)?;
    }

    return Ok(());
}

fn items_valid_recurse(repo: &Repository, item: &dyn RepoPublishable) -> Result<(), RepoError> {
    let valid = item.repo_valid(repo);
    let items = item.publishable_children();
    if valid.is_err() || items.is_none() { return valid; }

    for i in items.unwrap() {
        items_valid_recurse(repo, i)?;
    }

    return valid;
}

pub fn get_index(repo: &Repository) -> Result<Index, RepoError> {
    return match repo.index() {
        Ok(i) => Ok(i),
        Err(e) => Err(RepoError::GitErr(e, String::from("retrieve repo index")))
    };
}

pub fn clone(url: &str, dest: impl AsRef<Path>) -> Result<(), RepoError> {
    let thread_log = cmterm::Log::get();
    let git_auth = auth_git2::GitAuthenticator::new().set_prompter(LogHandle::new(thread_log.clone()))
                                     .add_default_ssh_keys()
                                     .try_cred_helper(true)
                                     .try_ssh_agent(true)
                                     .try_password_prompt(1)
                                     .prompt_ssh_key_password(true);
    
    return match git_auth.clone_repo(url, dest) {
        Ok(_) => Ok(()),
        Err(e) => {
            let err_msg = format!("Clone failed with error {e}");
            thread_log.log_err(&err_msg);
            Err(RepoError::CloneFailed(err_msg))
        }
    };
}

pub fn publish(repo: &Repository, item: &mut impl RepoPublishable, author: Option<String>, author_email: Option<String>) -> Result<(), RepoError> {
    let thread_log = cmterm::Log::get();

    let head = match repo.head() {
        Ok(h) => h,
        Err(_e) => return Err(RepoError::HeadCheckFailed(repo_errname(repo)))
    };

    match repo.head_detached() {
        Ok(detached) => if detached { return Err(RepoError::HeadDetached(repo_errname(repo))) },
        Err(_) => return Err(RepoError::HeadCheckFailed(repo_errname(repo)))
    };

    if !head.is_branch() {
        return Err(RepoError::HeadNotBranch(repo_errname(repo)));
    }

    let parent_commit = match head.peel_to_commit() {
        Ok(c) => c,
        Err(e) => return Err(RepoError::GitErr(e, String::from("resolve HEAD to commit")))
    };

    thread_log.log("Validating repository state...");
    items_valid_recurse(repo, item)?;

    let mut index = get_index(repo)?;

    match index.read(true) {
        Ok(_) => (),
        Err(e) => return Err(RepoError::GitErr(e, String::from("reset index to state on disk")))
    };

    match item.derivable_children() {
        Some(mut v) => {
            thread_log.log("Deriving repository items...");
            for d in v.iter_mut() {
                item_derive_recurse(repo, *d)?;
            }

            thread_log.log("Processing repository items...");
            for d in v.iter_mut() {
                item_process_recurse(repo, *d)?;
            }
        }
        None => ()
    };

    thread_log.log("Publishing repository items...");
    item_write_changes_recurse(repo, item).unwrap();

    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None).unwrap();
    let index_tree_oid = index.write_tree().unwrap();

    let index_tree = repo.find_tree(index_tree_oid).unwrap();

    let author = Signature::now(
        &author.unwrap_or(String::from("Codeless Mission Uploader")),
        &author_email.unwrap_or(String::from("91488389+Sprixitite@users.noreply.github.com"))
    ).unwrap();

    let committer = Signature::now("Codeless Mission Uploader", "91488389+Sprixitite@users.noreply.github.com").unwrap();

    let commit_oid = repo.commit(Some("HEAD"), &author, &committer, &item.publish_message(), &index_tree, &[&parent_commit]).unwrap();
    thread_log.log(format!("Commit Oid: {}", commit_oid.to_string()));
    
    // TODO: Is this even valid?
    // head.set_target(commit_oid, &item.publish_message())?;

    let target_remote = item.publish_target_remote(repo)?;
    let mut remote = repo.find_remote(&target_remote).unwrap();
    
    // let mut cred_helper = git2::CredentialHelper::new(remote.url().unwrap());
    // cred_helper.config( git2::Config:: )

    // let a = PushOptions::new();
    // let b = RemoteCallbacks::new();
    let git_auth = auth_git2::GitAuthenticator::new().set_prompter(LogHandle::new(thread_log.clone()))
                                     .add_default_ssh_keys()
                                     .try_cred_helper(true)
                                     .try_ssh_agent(true)
                                     .try_password_prompt(1)
                                     .prompt_ssh_key_password(true);

    match git_auth.push(repo, &mut remote, &[head.name().unwrap()]) {
        Ok(_) => (),
        Err(e) => return Err(RepoError::GitErr(e, String::from("when pushing to remote")))
    };

    match index.clear() {
        Ok(_) => (),
        Err(e) => return Err(RepoError::GitErr(e, String::from("when clearing index")))
    };

    thread_log.log("Copying link to clipboard...");

    let content_url = format!(
        "{}/raw/{}/{}", 
        remote.url().expect("remote URL should be valid").replace("gist.github.com", "gist.githubusercontent.com").trim_end_matches("/"),
        commit_oid.to_string(),
        item.publish_target_file()
    );

    match crate::clipboard::set_text(content_url) {
        Ok(_) => thread_log.log_success("Copied link to clipboard"),
        Err(e) => {
            thread_log.log_err(format!("Error whilst copying to clipboard {:?}", e));
            panic!("Sprix couldn't be bothered implementing proper error handling for this and would just like to eat")
        }
    }

    return Ok(());
}