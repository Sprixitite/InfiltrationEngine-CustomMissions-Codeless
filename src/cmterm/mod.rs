use std::{sync::{mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError}, Arc, Condvar, Mutex}, thread::{self, JoinHandle}, time::Duration};
use console::Term;

mod input;
mod log;
mod render;
mod ring_buffer;

pub use input::Input;
pub use log::{Log, LogHandle};
pub use render::Renderable;

use crate::cmterm::{render::Renderer};

struct _Manager {

}

pub struct Manager {
    pub main_log: Arc<Log>,
    pub server_log: Arc<Log>,
    pub term_input_lowp: Arc<Input>,
    pub term_input_highp: Arc<Input>,

    #[allow(dead_code)] // Can be used by consumers to force a re-render
    pub render_send: Sender<()>,
    render_recv: Receiver<()>,

    input_send_highp: Sender<console::Key>,
    input_send_lowp: Sender<console::Key>
}

impl Manager {
    pub fn new() -> Self {
        let (render_s, render_r) = channel();

        // High + Low priority input channels
        let (isend_highp, irecv_highp) = channel();
        let (isend_lowp, irecv_lowp) = channel();

        let input_lowp = Arc::new(Input::new(render_s.clone(), irecv_lowp));
        let input_highp = Arc::new(Input::new(render_s.clone(), irecv_highp));

        return Manager {
            main_log: Arc::new(Log::new("Main Thread", input_lowp.clone())),
            server_log: Arc::new(Log::new("Server Thread", input_highp.clone())),
            term_input_lowp: input_lowp,
            term_input_highp: input_highp,
            render_recv: render_r,
            render_send: render_s,
            input_send_lowp: isend_lowp,
            input_send_highp: isend_highp
        };
    }

    fn render_loop(self, kill_recv: Receiver<()>, interval: Duration) -> Self {
        loop {
            match self.render_recv.recv_timeout(interval) {
                Ok(_) => (),
                Err(e) => match e {
                    RecvTimeoutError::Timeout => (),
                    RecvTimeoutError::Disconnected => return self
                }
            }

            let highp_in_use = self.term_input_highp.is_inputting();
            self.term_input_lowp.set_disabled(highp_in_use);

            //self.main_log.log_warn(format!("Set term_input_lowp.termread_valid = {}", !highp_in_use));
            

            Renderer.render(&Term::stderr(), &self).expect("Render shouldn't fail");

            match kill_recv.try_recv() {
                Ok(_) => return self,
                Err(e) => match e {
                    TryRecvError::Empty => (),
                    TryRecvError::Disconnected => return self
                }
            }
        }
    }

    fn input_loop(kill_recv: Receiver<()>, senders: Vec<Sender<console::Key>>) {
        let delay = Duration::from_millis(500);

        let key_read = Arc::new(Mutex::new(false));
        let key_read_condvar = Arc::new(Condvar::new());

        loop {
            let key_read_2 = key_read.clone();
            let key_read_condvar_2 = key_read_condvar.clone();

            let mut key_read_success = key_read.lock().unwrap();
            *key_read_success = false;

            let readkey_thread = thread::Builder::new().name(String::from("input readkey")).spawn(move || {
                let key = Term::stderr().read_key();
                *(key_read_2.lock().unwrap()) = true;
                key_read_condvar_2.notify_all();
                return key;
            }).unwrap();

            let mut disconnect = false;
            while !(*key_read_success) && !disconnect {
                key_read_success = match key_read_condvar.wait_timeout(key_read_success, delay) {
                    Ok((krs, _wtr)) => krs,
                    Err(e) => panic!("PoisonError {:?} in input readkey thread!", e)
                };

                match kill_recv.try_recv() {
                    Ok(_) => { disconnect = true; },
                    Err(e) => match e {
                        TryRecvError::Disconnected => { disconnect = true; },
                        TryRecvError::Empty => continue
                    }
                }
            };

            if disconnect { return }

            let readkey_result = match readkey_thread.join() {
                Ok(s) => s,
                Err(e) => panic!("Readkey thread panicked with {:?}", e)
            };
            
            let key = match readkey_result {
                Ok(k) => k,
                Err(e) => panic!("Failed to read key with {:?}", e),
            };

            for sender in &senders {
                sender.send(key.clone()).unwrap();
            }
        }
    }

    pub fn spawn_threads(self, redraw_interval: u64) -> (Sender<()>, JoinHandle<Manager>) {
        // Consume all early messages
        self.render_recv.try_iter().count();
        let redraw_interval = std::time::Duration::from_millis(redraw_interval);

        let input_senders = vec![self.input_send_highp.clone(), self.input_send_lowp.clone()];

        let (rkill_send, rkill_recv) = channel();
        let (ikill_send, ikill_recv) = channel();
        let (kkill_send, kkill_recv) = channel();

        let render_join = thread::Builder::new().name(String::from("render")).spawn(move || {
            return self.render_loop(rkill_recv, redraw_interval);
        }).unwrap();

        let input_join = thread::Builder::new().name(String::from("input")).spawn(move || {
            return Manager::input_loop(ikill_recv, input_senders);
        }).unwrap();

        let kill_join = thread::Builder::new().name(String::from("render/input kill")).spawn(move || {
            let _result = kkill_recv.recv();
            ikill_send.send(()).expect("input thread kill shouldn't have hung up");
            rkill_send.send(()).expect("render thread kill shouldn't have hung up");
            input_join.join().expect("input thread should've exited gracefully");
            return render_join.join().unwrap();
        }).unwrap();

        return (kkill_send, kill_join);
    }
}

impl Renderable for Manager {
    fn get_log_bufs(&self) -> Vec<&Log> {
        return vec![&self.main_log, &self.server_log];
    }

    fn get_input_handler(&self) -> &Input {
        match self.term_input_highp.is_inputting() {
            true => &self.term_input_highp,
            false => &self.term_input_lowp
        }
    }
}