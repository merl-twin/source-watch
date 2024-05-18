use std::{
    io, fs,
    io::Read,
    sync::Arc,
    path::{Path,PathBuf},
    time::{SystemTime,Duration},
};


use crossbeam::{
    queue::ArrayQueue,
    channel::{self,Sender,RecvTimeoutError},
};

use crate::{
    WatcherError,
};

lazy_static::lazy_static! {
    pub(crate) static ref RES_MANAGER: ResourceManager = ResourceManager::new();
}

enum Message {
    Start,
    Stop,
    Interval(Duration),
    Inner(TextWatcherInner),
}

pub(crate) struct ResourceManager {
    sender: Option<Sender<Message>>,
    handle: Option<std::thread::JoinHandle<()>>,
}
impl Drop for ResourceManager {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(h) = self.handle.take() {
            h.join().ok();
        }
    }
}
impl ResourceManager {
    fn new() -> ResourceManager {
        let mut int = std::time::Duration::new(1,0);
        let mut do_watch = true;
        let (tx,rx) = channel::unbounded();
        ResourceManager {
            sender: Some(tx),
            handle: Some(std::thread::spawn(move || {
                let mut inners = Vec::new();
                loop {
                    match rx.recv_timeout(int) {
                        Ok(message) => match message {
                            Message::Start => do_watch = true,
                            Message::Stop => do_watch = false,
                            Message::Interval(i) => int = i,
                            Message::Inner(inner) => inners.push(inner),
                        },
                        Err(RecvTimeoutError::Disconnected) => break,
                        Err(RecvTimeoutError::Timeout) => {}, 
                    }
                    while let Ok(message) = rx.try_recv() {
                        match message {
                            Message::Start => do_watch = true,
                            Message::Stop => do_watch = false,
                            Message::Interval(i) => int = i,
                            Message::Inner(inner) => inners.push(inner),
                        }
                    }
                    if do_watch {
                        for inner in &mut inners {
                            inner.check_update();
                        }
                    }
                }
            })),
        }
    }
    pub fn stop(&self) -> Result<(),WatcherError> {
        match &self.sender {
            None => Err(WatcherError::DeadWatcher),
            Some(sender) => match sender.send(Message::Stop) {
                Ok(()) => Ok(()),
                Err(_) => Err(WatcherError::DeadWatcher),
            },
        }
    }
    pub fn start(&self) -> Result<(),WatcherError> {
        match &self.sender {
            None => Err(WatcherError::DeadWatcher),
            Some(sender) => match sender.send(Message::Start) {
                Ok(()) => Ok(()),
                Err(_) => Err(WatcherError::DeadWatcher),
            },
        }
    }    
    pub fn interval(&self, int: std::time::Duration) -> Result<(),WatcherError> {
        match &self.sender {
            None => Err(WatcherError::DeadWatcher),
            Some(sender) => match sender.send(Message::Interval(int)) {
                Ok(()) => Ok(()),
                Err(_) => Err(WatcherError::DeadWatcher),
            },
        }
    }
    pub(crate) fn register<P: AsRef<Path>>(&self, path: P) -> Result<(String,TextResourceInner),WatcherError> {
        let (text,rc,inner) = recource_with_updates(path.as_ref()).map_err(WatcherError::Io)?;
        match &self.sender {
            None => Err(WatcherError::DeadWatcher),
            Some(sender) => match sender.send(Message::Inner(inner)) {
                Ok(()) => Ok((text,rc)),
                Err(_) => Err(WatcherError::DeadWatcher),
            },
        }
    }
}


fn recource_with_updates(path: &Path) -> Result<(String,TextResourceInner,TextWatcherInner),io::Error> {
    let path = Arc::new(path.to_path_buf());
    let tm = fs::metadata(path.as_ref())?.modified()?;    
    let mut text = String::new();
    io::BufReader::new(fs::File::open(path.as_ref())?)
        .read_to_string(&mut text)?;
    let queue = Arc::new(ArrayQueue::new(1));
    Ok((text,TextResourceInner{ path: path.clone(), updates: queue.clone() }, TextWatcherInner { path, tm, sender: queue }))
}

#[derive(Debug,Clone)]
pub(crate) struct TextResourceInner {
    path: Arc<PathBuf>,
    updates: Arc<ArrayQueue<Result<String,io::Error>>>,
}
impl TextResourceInner {
    pub(crate) fn update(&self) -> Option<Result<String,io::Error>> {        
        self.updates.pop()
    }
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

struct TextWatcherInner {
    path: Arc<PathBuf>,
    tm: SystemTime,
    sender: Arc<ArrayQueue<Result<String,io::Error>>>,
}
impl TextWatcherInner {
    fn check_update_inner(&mut self) -> Result<Option<String>,io::Error> {
        let tm = fs::metadata(self.path.as_ref())?.modified()?;
        Ok(match tm > self.tm {
            true => {
                self.tm = tm;            
                let mut text = String::new();
                io::BufReader::new(fs::File::open(self.path.as_ref())?)
                    .read_to_string(&mut text)?;
                Some(text)
            },
            false => None,
        })
    }
    
    pub(crate) fn check_update(&mut self) {
        match self.check_update_inner() {
            Ok(None) => {},
            Ok(Some(text)) => {
                self.sender.force_push(Ok(text));
            },
            Err(e) => {
                self.sender.force_push(Err(e));
            },
        }
    }        
}

