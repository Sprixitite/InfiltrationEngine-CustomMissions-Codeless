use std::{cell::RefCell, fs::{File, OpenOptions}, io::{self, Write}, ops::Deref, path::Path, sync::{Arc, LazyLock, Mutex}};

use console::style;

use super::{ring_buffer::RingBuffer, input::Input};

thread_local! {
    static THREAD_LOGGER: RefCell<Option<Arc<Log>>> = const { RefCell::new(None) };
}

#[derive(Clone)]
pub(super) struct _TerminalLogData {
    pub(super) title: String,
    pub(super) lines: RingBuffer<String, 256>,
    pub(super) disk_log_path: Option<String>,
}

pub struct Log {
    pub(super) data: Mutex<_TerminalLogData>,
    input: Arc<Input>
}

#[derive(Clone)]
pub struct LogHandle {
    inner: Arc<Log>
}

impl LogHandle {
    pub fn new(inner: Arc<Log>) -> Self {
        return LogHandle { inner: inner }
    }
}

impl Log {
    pub fn new(name: impl Into<String>, input: Arc<Input>) -> Self {
        let title = name.into();
        return Log {
            data: Mutex::new(
                _TerminalLogData { 
                    title: title.clone(),
                    lines: RingBuffer::new(),
                    disk_log_path: None
                }
            ), //.with_name(format!("{}.log_data", title)),
            input: input
        }
    }

    #[allow(unused)]
    pub fn with_disk_log(mut self, path: impl Into<String>) -> Self {
        self.data.lock().unwrap().disk_log_path = Some(path.into());
        return self;
    }

    pub fn log(&self, msg: impl AsRef<str>) {
        static PREFIX: LazyLock<String> = LazyLock::new(|| {
            style("   INFO:").bold().black().on_white().to_string()
        });

        return self._log(
            msg,
            PREFIX.deref(),
            |s: &str| {
                return style(s).white().to_string();
            }
        );
    }

    pub fn log_warn(&self, msg: impl AsRef<str>) {
        static PREFIX: LazyLock<String> = LazyLock::new(|| {
            style("   WARN:").bold().black().on_yellow().to_string()
        });

        return self._log(
            msg,
            PREFIX.deref(),
            |s: &str| {
                return style(s).yellow().to_string();
            }
        );
    }

    pub fn log_err(&self, msg: impl AsRef<str>) {
        static PREFIX: LazyLock<String> = LazyLock::new(|| {
            style("  ERROR:").bold().white().on_red().to_string()
        });

        return self._log(
            msg,
            PREFIX.deref(),
            |s: &str| {
                return style(s).red().to_string();
            }
        );
    }

    pub fn log_success(&self, msg: impl AsRef<str>) {
        static PREFIX: LazyLock<String> = LazyLock::new(|| {
            style("SUCCESS:").bold().black().on_green().to_string()
        });

        return self._log(
            msg,
            PREFIX.deref(),
            |s: &str| {
                return style(s).green().to_string();
            }
        );
    }

    fn _log(&self, msg: impl AsRef<str>, prefix: impl AsRef<str>, styler: fn(&str) -> String) {
        self._file_log(msg.as_ref());

        let prefix_empty = " ".repeat(console::measure_text_width(prefix.as_ref()));
        let msg_lines: Vec<&str> = msg.as_ref().lines().collect();
        let mut current_prefix = prefix.as_ref();

        let mut data = self.data.lock().unwrap();
        for line in msg_lines {
            let line_styled = styler(&line);
            let final_line = format!("{} {}", current_prefix, line_styled).replace('\t', "  ");
            data.lines.push(final_line);
            current_prefix = &prefix_empty;
        }
        
        return;
    }

    fn _file_log(&self, msg: impl AsRef<[u8]>) {
        match self.get_disk_log(OpenOptions::new().create(true).append(true)) {
            Ok(o) => match o {
                Some(mut f) => {
                    let _ = f.write(msg.as_ref());
                },
                None => ()
            },
            Err(_) => (),
        }
    }

    pub fn get_disk_log(&self, options: &OpenOptions) -> io::Result<Option<File>> {
        let data = self.data.lock().unwrap();
        if data.disk_log_path.is_none() { return Ok(None); }

        let file_path = Path::new(&data.disk_log_path.as_ref().unwrap()).join(&data.title);
        return options.open(file_path).map(|f| { Some(f) });
    }

    pub fn wait_for_enter(&self, prompt: impl Into<String>) -> io::Result<()> {
        self.input.wait_for_enter(self.name(), prompt)?;
        return Ok(());
    }

    pub fn request_string(&self, prompt: impl Into<String>) -> io::Result<String> {
        return self.input.request_string(self.name(), prompt);
    }

    pub fn request_password(&self, prompt: impl Into<String>) -> io::Result<String> {
        return self.input.request_password(self.name(), prompt)
    }

    pub fn name(&self) -> String {
        return self.data.lock().unwrap().title.clone();
    }

    fn thread_name() -> String {
        let thread = std::thread::current();
        return (&thread).name().map_or(format!("{:?}", thread.id()), |s| { s.to_string() });
    }

    /// Initializes the logger for the current thread
    pub fn set(to: Arc<Log>) {
        THREAD_LOGGER.with_borrow_mut(|opt| {
            match opt {
                Some(s) => {
                    if Arc::ptr_eq(&to, s) { return }
                    let warn_msg = format!("Attempt to set logger as thread logger for already initialized thread {}", Log::thread_name());
                    to.log_warn(&warn_msg);
                    s.log_warn(&warn_msg);
                },
                None => *opt = Some(to),
            }
        });
    }

    pub fn get() -> Arc<Log> {
        let ret_val = THREAD_LOGGER.with_borrow(|opt| {
            opt.clone()
        });

        let ret_val = match ret_val {
            Some(l) => l,
            None => panic!("Attempt to get log on thread with no initialized log!")
        };

        return ret_val;
    }
}

impl auth_git2::Prompter for LogHandle {
    fn prompt_username_password(
        &mut self,
        _url: &str,
        _git_config: &git2::Config,
    ) -> Option<(String, String)> {
        let username = match self.inner.request_string("Enter Git Username // ") {
            Ok(s) => Some(s),
            Err(_e) => None
        };

        let password = match self.inner.request_password("Enter Git Password // ") {
            Ok(s) => Some(s),
            Err(_e) => None
        };

        return match username {
            Some(user) => match password {
                Some(pwd) => Some((user, pwd)),
                None => None
            },
            None => None
        };
    }

    fn prompt_password(
        &mut self,
        username: &str,
        _url: &str,
        _git_config: &git2::Config,
    ) -> Option<String> {
        return match self.inner.request_password(format!("Enter Git Password [{}] // ", &username)) {
            Ok(pwd) => Some(pwd),
            Err(_e) => None
        }
    }

    fn prompt_ssh_key_passphrase(
        &mut self,
        private_key_path: &Path,
        _git_config: &git2::Config,
    ) -> Option<String> {
        return match self.inner.request_password(format!("Enter SSH Key Passphrase [{:.32}] // ", &private_key_path.to_string_lossy())) {
            Ok(passkey) => Some(passkey),
            Err(_e) => None
        }
    }
}