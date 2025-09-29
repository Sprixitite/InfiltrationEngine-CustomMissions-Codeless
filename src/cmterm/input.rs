use std::{io, sync::{atomic::{AtomicBool, Ordering}, mpsc::{Receiver, Sender}, Mutex}, thread, time::Duration};

use console::Key;

#[derive(Clone)]
pub(super) struct _InputData {
    pub(super) input_pos: usize,
    pub(super) input_buffer: String,
    pub(super) input_prompt: String,
    pub(super) input_requester: String
}

pub struct Input {
    // Free if there is currently no ongoing input
    inputting: Mutex<Receiver<console::Key>>,

    // Free unless being written to by an input
    pub(super) input_state: Mutex<_InputData>,
    //pub(super) input_state_changed: Condvar,
    
    //pub(super) name: String,
    
    request_redraw: Sender<()>,

    input_disabled: AtomicBool,
    input_in_use: AtomicBool,
}

impl Input {
    pub(super) fn new(request_redraw: Sender<()>, input_recv: Receiver<Key>) -> Input {
        return Input {
            inputting: Mutex::new(input_recv), //.with_name(format!("{}.inputting", &name)),
            //input_state_changed: Condvar::new(),
            input_state: Mutex::new(
                _InputData {
                    input_pos: 0, 
                    input_buffer: String::with_capacity(128), 
                    input_prompt: String::new(), 
                    input_requester: String::new() 
                }
            ), //.with_name(format!("{}.input_data", &name)),
            input_disabled: AtomicBool::new(false),
            input_in_use: AtomicBool::new(false),
            request_redraw: request_redraw,
            //name: name,
        }
    }

    pub fn set_inputting(&self, to: bool) {
        self.input_in_use.store(to, Ordering::Release);
    }

    pub fn is_inputting(&self) -> bool {
        return self.input_in_use.load(Ordering::Acquire);
    }

    pub fn set_disabled(&self, to: bool) {
        self.input_disabled.store(to, Ordering::Release);
    }

    pub fn is_disabled(&self) -> bool {
        return self.input_disabled.load(Ordering::Acquire);
    }

    fn wait_for_enabled(&self) {
        while self.is_disabled() {
            thread::park_timeout(Duration::from_secs(1));
        }
        // let input_state = self.input_state.lock().unwrap();
        // let _unused = self.input_state_changed.wait_while(input_state, |v| { v.input_disable }).unwrap();
    }

    fn request_input(
        &self,
        thread_name: impl Into<String>,
        prompt: impl Into<String>,
        input_fn: fn(&Input, &Receiver<Key>) -> io::Result<String>
    ) -> io::Result<String> {
        let input_recv = self.inputting.lock().unwrap();

        // Consume all pending values from before the current input
        // Program loses its shit without this line
        input_recv.try_iter().count();
        
        let thread_name = thread_name.into();
        {
            let mut input_info = self.input_state.lock().unwrap();
            let prompt_str = prompt.into();
            input_info.input_requester = thread_name.clone();
            input_info.input_pos = 0;
            input_info.input_prompt = prompt_str;
            input_info.input_buffer = String::new();
        }
        self.set_inputting(true);
        self.request_redraw.send(()).unwrap();
        let result = input_fn(self, &input_recv);
        self.set_inputting(false);
        
        self.request_redraw.send(()).unwrap();
        return result;
    }
    
    pub fn request_string(&self, thread_name: impl Into<String>, prompt: impl Into<String>) -> io::Result<String> {
        return self.request_input(thread_name, prompt, Input::_wait_for_string);
    }
    
    pub fn wait_for_enter(&self, thread_name: impl Into<String>, prompt: impl Into<String>) -> io::Result<()> {
        return self.request_input(thread_name, prompt, Input::_wait_for_enter).map(|_| { () });
    }

    //pub fn request_multiselect(&self, thread_name: impl Into<String>, prompt: impl Into<String>) -> io::Result<i32> {}
    
    fn _wait_for_string(&self, input_recv: &Receiver<Key>) -> io::Result<String> {
        loop {
            self.wait_for_enabled();
            self.request_redraw.send(()).unwrap();
            match self._read_char(input_recv)? {
                Some(k) => match k {
                    Key::Char(c) => {
                        let mut input_data = self.input_state.lock().unwrap();
                        input_data.input_buffer.push(c);
                        input_data.input_pos += 1;
                    },
                    
                    Key::Enter => {
                        break
                    }
                    
                    Key::Backspace => {
                        let mut input_data = self.input_state.lock().unwrap();
                        if input_data.input_buffer.pop().is_some() {
                            input_data.input_pos -= 1;
                        }
                    }
                    
                    _ => ()
                }
                None => ()
            }
        }
        
        let input_buf = { self.input_state.lock().unwrap().input_buffer.clone() };
        return Ok(input_buf);
    }

    fn _wait_for_enter(&self, input_recv: &Receiver<Key>) -> io::Result<String> {

        loop {
            self.wait_for_enabled();
            self.request_redraw.send(()).unwrap();
            match self._read_char(input_recv)? {
                Some(k) => match k {
                    Key::Enter => break,
                    _ => ()
                },
                None => ()
            }
        }

        return Ok(String::new())
    }

    pub fn request_password(&self, thread_name: impl Into<String>, prompt: impl Into<String>) -> io::Result<String> {
        return self.request_input(thread_name, prompt, Input::_wait_for_password);
    }

    fn _wait_for_password(&self, input_recv: &Receiver<Key>) -> io::Result<String> {
        let mut password = String::with_capacity(64);
        loop {
            self.wait_for_enabled();
            self.request_redraw.send(()).unwrap();
            match self._read_char(input_recv)? {
                Some(k) => match k {
                    Key::Char(c) => {
                        let mut input_data = self.input_state.lock().unwrap();
                        input_data.input_buffer.push('*');
                        input_data.input_pos += 1;
                        password.push(c);
                    },
                    
                    Key::Enter => {
                        break
                    }
                    
                    Key::Backspace => {
                        let mut input_data = self.input_state.lock().unwrap();
                        if input_data.input_buffer.pop().is_some() {
                            input_data.input_pos -= 1;
                            password.pop();
                        }
                    }
                    
                    _ => ()
                }
                None => ()
            }
        }
        
        let input_buf = { self.input_state.lock().unwrap().input_buffer.clone() };
        return Ok(input_buf);
    }
    
    fn _read_char(&self, recv: &Receiver<Key>) -> io::Result<Option<Key>> {
        let key = recv.recv().unwrap();
        let termread_valid = !self.is_disabled();
        
        return Ok(match termread_valid {
            true => Some(key),
            false => None
        })
    }
}