use std::{error::Error, fmt::Display};

use git2::Repository;

use crate::repo_management::{self, RepoDerivable, RepoError, RepoItem, RepoPublishable};

const CODELESS_CM_IDENTIFIER: &'static str = "_infilengine_cm_codeless_";
const CODELESS_ELEM_DELIMIT: &'static str = "|";

fn next_code_elem(code: &str, fail_err: MissionCodeParseError) -> Result<(&str, &str), MissionCodeParseError> {
    return match code.split_once(CODELESS_ELEM_DELIMIT) {
        Some(s) => Ok(s),
        None => Err(fail_err)
    };
}

#[derive(Debug)]
pub enum MissionCodeParseError {
    CodelessVersionUnknown(usize),
    CodelessVersionMissing,
    CodelessVersionNotUInt,

    FeatureCountMissing,
    FeatureCountInvalid,
    FeatureMissing,

    GistFileMissing,
    GistRemoteMissing,
    GistURLMissing,

    HasBothGistRemoteAndURL,
    HasNoGistRemoteOrURL,

    InputWasntCode,
}

impl Display for MissionCodeParseError { 
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CodelessVersionUnknown(v) => f.write_fmt(format_args!("input strings version \'{}\' is not supported", v)),
            Self::CodelessVersionMissing => f.write_str("input string is missing codeless version"),
            Self::CodelessVersionNotUInt => f.write_str("input strings version was not a uint"),
            
            Self::FeatureCountMissing => f.write_str("input string is missing feature count"),
            Self::FeatureCountInvalid => f.write_str("input string feature count wasn't valid u64"),
            Self::FeatureMissing => f.write_str("input string is missing expected feature"),

            Self::GistFileMissing => f.write_str("input string is missing gist filename"),
            Self::GistRemoteMissing => f.write_str("input string is missing remote"),
            Self::GistURLMissing => f.write_str("input string is missing gist URL"),

            Self::HasBothGistRemoteAndURL => f.write_str("input string has both a gist remote and a gist URL"),
            Self::HasNoGistRemoteOrURL => f.write_str("input string has neither a gist remote or gist URL"),

            Self::InputWasntCode => f.write_str("input string was not valid codeless mission"),
        }
    }
}

impl Error for MissionCodeParseError { }

pub enum CodelessInfo {
    V0
}

impl CodelessInfo {
    pub fn version(&self) -> usize {
        return match self {
            CodelessInfo::V0 => 0
        };
    }

    pub fn parse_from(code: &str) -> Result<(CodelessInfo, &str), MissionCodeParseError> {
        let (version, code) = next_code_elem(code, MissionCodeParseError::CodelessVersionMissing)?;
        let version = match version.parse::<usize>() {
            Ok(u) => u,
            Err(_) => return Err(MissionCodeParseError::CodelessVersionNotUInt)
        };

        let ci = match version {
            0 => CodelessInfo::V0,
            _ => return Err(MissionCodeParseError::CodelessVersionUnknown(version))
        };

        return Ok((ci, code));
    }
}

impl RepoItem for CodelessInfo {
    fn derivable_children(&mut self) -> Option<Vec<&mut dyn RepoDerivable>> {
        return None;
    }

    fn publishable_children(&self) -> Option<Vec<&dyn RepoPublishable>> {
        return None;
    }
}

impl RepoPublishable for CodelessInfo {
    fn repo_publish(&self, repo: &Repository) -> Result<(), RepoError> { return Ok(()); }
    fn repo_valid(&self, repo: &Repository) -> Result<(), RepoError> { return Ok(()); }
}

pub enum CodelessRepoFeature {
    UnknownFeature(String),
    MissionVersion(u64)
}

impl CodelessRepoFeature {
    fn from_str(feature_str: &str) -> CodelessRepoFeature {
        return match feature_str {
            "MissionVersion" => Self::MissionVersion(0),
            _ => Self::UnknownFeature(feature_str.to_string())
        }
    }
}

impl Display for CodelessRepoFeature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return match self {
            CodelessRepoFeature::MissionVersion(_) => f.write_str("MissionVersion"),
            CodelessRepoFeature::UnknownFeature(s) => f.write_fmt(format_args!("Unknown[{}]", s))
        }
    }
}

impl RepoItem for CodelessRepoFeature {
    fn derivable_children(&mut self) -> Option<Vec<&mut dyn RepoDerivable>> {
        return None;
    }

    fn publishable_children(&self) -> Option<Vec<&dyn RepoPublishable>> {
        return None;
    }
}

impl RepoPublishable for CodelessRepoFeature {
    fn repo_publish(&self, repo: &Repository) -> Result<(), RepoError> {
        return match self {
            CodelessRepoFeature::MissionVersion(v) => {
                repo_management::overwrite_file(repo, ".custommissionversion", &v.to_string())?;
                Ok(())
            }
            CodelessRepoFeature::UnknownFeature(f) => {
                Ok(())
            }
        }
    }

    fn repo_valid(&self, repo: &Repository) -> Result<(), RepoError> {
        return match self {
            CodelessRepoFeature::MissionVersion(_) => {
                let p = repo_management::get_repo_file_path(repo, ".custommissionversion")?;
                match p.exists() && !p.is_file() {
                    true => Err(RepoError::PublishError(format!("non-file item already exists at {}", p.to_string_lossy()))),
                    false => Ok(())
                }
            }
            CodelessRepoFeature::UnknownFeature(_) => Ok(())
        }
    }
}

impl RepoDerivable for CodelessRepoFeature {
    fn repo_derive(&mut self, repo: &Repository) -> Result<(), RepoError> {
        return match self {
            CodelessRepoFeature::MissionVersion(v) => {
                let version_string = repo_management::read_file(repo, ".custommissionversion")?;
                *v = match version_string.parse::<u64>() {
                    Ok(rv) => rv,
                    Err(e) => return Err(RepoError::DeriveError(format!(".custommissionversion file did not contain valid u64 with error {}", e)))
                };
                Ok(())
            }
            CodelessRepoFeature::UnknownFeature(_) => Ok(())
        }
    }

    fn repo_process(&mut self, repo: &Repository) -> Result<(), RepoError> {
        return match self {
            CodelessRepoFeature::MissionVersion(v) => {
                *v += 1;
                Ok(())
            }
            CodelessRepoFeature::UnknownFeature(_) => Ok(())
        }
    }
}

pub struct MissionCode {
    pub codeless_fmt_version: CodelessInfo,
    pub codeless_features: Vec<CodelessRepoFeature>,
    pub gist_file: String,
    pub gist_url: Option<String>,
    pub gist_remote: Option<String>,
    pub code_data: String,
}

impl MissionCode {
    pub fn parse_from(code: &str) -> Result<MissionCode, MissionCodeParseError> {
        if !code.starts_with(CODELESS_CM_IDENTIFIER) { return Err(MissionCodeParseError::InputWasntCode) }

        // +1 for delimiter
        let code = &code[CODELESS_CM_IDENTIFIER.len()+1..];

        let (codeless_info, code) = CodelessInfo::parse_from(code)?;
        
        let (feature_count_str, code) = next_code_elem(code, MissionCodeParseError::FeatureCountMissing)?;
        let feature_count = match feature_count_str.parse::<usize>() {
            Ok(c) => c,
            Err(_) => return Err(MissionCodeParseError::FeatureCountInvalid)
        };

        let mut code = code;
        let mut feature_vec = Vec::<CodelessRepoFeature>::with_capacity(feature_count);
        for _ in 0..feature_count {
            let (feature_str, code_slice) = next_code_elem(code, MissionCodeParseError::FeatureMissing)?;
            code = code_slice;

            let feature = CodelessRepoFeature::from_str(feature_str);
            feature_vec.push(feature);
        }

        let (gist_file, code) = next_code_elem(code, MissionCodeParseError::GistFileMissing)?;
        let (gist_url, code) = next_code_elem(code, MissionCodeParseError::GistURLMissing)?;
        let (gits_remote, code) = next_code_elem(code, MissionCodeParseError::GistRemoteMissing)?;

        let gist_url = match gist_url {
            "None" => None,
            _ => Some(gist_url.to_string())
        };

        let gist_remote = match gits_remote {
            "None" => None,
            _ => Some(gits_remote.to_string())
        };

        if gist_remote.is_some() && gist_url.is_some() {
            return Err(MissionCodeParseError::HasBothGistRemoteAndURL)
        } else if gist_remote.is_none() && gist_url.is_none() {
            return Err(MissionCodeParseError::HasNoGistRemoteOrURL)
        }

        let content = code;

        return Ok(MissionCode { 
            codeless_fmt_version: codeless_info,
            codeless_features: feature_vec,
            gist_file: gist_file.to_string(),
            gist_url: gist_url,
            gist_remote: gist_remote,
            code_data: content.to_string()
        });
    }

    pub fn feature_display(&self) -> String {
        let mut feature_strs = Vec::<String>::with_capacity(self.codeless_features.len());

        let mut first = true;
        let mut final_len = 0;
        for f in &self.codeless_features {
            let f_str = f.to_string();
            final_len += f_str.len();
            feature_strs.push(f_str);
            if !first { final_len += 2 }
            first = false;
        }

        first = true;
        let mut feat_str = String::with_capacity(final_len);
        for f in feature_strs {
            if !first { feat_str.push_str(", "); }
            feat_str.push_str(&f);
            first = false;
        }

        return feat_str;
    }
}

impl RepoItem for MissionCode {
    fn derivable_children(&mut self) -> Option<Vec<&mut dyn RepoDerivable>> {
        let mut derivable = Vec::<&mut dyn RepoDerivable>::new();
        derivable.reserve_exact(self.codeless_features.len());

        for feature in self.codeless_features.iter_mut() {
            derivable.push(feature);
        }

        return Some(derivable);
    }

    fn publishable_children(&self) -> Option<Vec<&dyn RepoPublishable>> {
        let mut publishable = Vec::<&dyn RepoPublishable>::new();
        publishable.reserve_exact(self.codeless_features.len()+1);

        publishable.push(&self.codeless_fmt_version);
        for feature in self.codeless_features.iter() {
            publishable.push(feature);
        }

        return Some(publishable);
    }
}

impl RepoPublishable for MissionCode {
    fn publish_target_file(&self) -> String {
        self.gist_file.clone()
    }

    fn publish_target_remote(&self, repo: &Repository) -> Result<String, RepoError> {
        match self.gist_remote.as_ref() {
            Some(remote) => return Ok(remote.clone()),
            None => ()
        };

        match self.gist_url.as_ref() {
            Some(url) => match repo_management::remote_name_from_url(repo, url)? {
                Some(remote) => return Ok(remote),
                None => return Err(RepoError::PublishError(format!("MissionCode remote URL did not match up with ")))
            },
            None => ()
        }

        return Err(RepoError::PublishError(String::from("MissionCode is invalid - has both remote name & URL")));
    }

    fn publish_message(&self) -> String {
        let mission_version = self.codeless_features.iter().find_map(|f| {
            match f {
                CodelessRepoFeature::MissionVersion(v) => Some(*v),
                _ => None
            }
        });

        return match mission_version {
            Some(v) => format!("Update To Newest Version - v{}", v),
            None => String::from("Update To Newest Version - Untracked")
        };
    }

    fn repo_publish(&self, repo: &Repository) -> Result<(), RepoError> {
        repo_management::overwrite_file(repo, &self.gist_file, &self.code_data)?;
        return Ok(())
    }

    fn repo_valid(&self, repo: &Repository) -> Result<(), RepoError> {
        // Valid if gist file exists && remote matches
        let p = repo_management::get_repo_file_path(repo, &self.gist_file)?;
        match p.exists() && !p.is_file() {
            true => return Err(RepoError::PublishError(format!("non-file item already exists at {}", self.gist_file))),
            false => ()
        };

        let repo_is_valid = match self.gist_url.as_ref() {
            Some(url) => repo_management::has_remote_url(repo, url)?,
            None => match self.gist_remote.as_ref() {
                Some(remote) => repo_management::has_remote(repo, remote)?,
                None => return Err(RepoError::PublishError(String::from("mission code is missing both remote name and URL")))
            }
        };

        return match repo_is_valid {
            true => Ok(()),
            false => match self.gist_url.is_some() {
                true => Err(RepoError::PublishError(format!("repo is missing remote URL {}", self.gist_url.as_ref().unwrap()))),
                false => Err(RepoError::PublishError(format!("repo is missing remote name {}", self.gist_remote.as_ref().unwrap())))
            }
        }
    }
}