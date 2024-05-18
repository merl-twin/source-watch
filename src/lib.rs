mod resources;


use resources::{
    RES_MANAGER,
    TextResourceInner,
};

use std::{
    path::Path,
};

pub fn stop_watch() -> Result<(),WatcherError> {
    RES_MANAGER.stop()
}

pub fn start_watch() -> Result<(),WatcherError> {
    RES_MANAGER.start()
}

pub fn watch_interval(int: std::time::Duration) -> Result<(),WatcherError> {
    RES_MANAGER.interval(int)
}

#[derive(Debug)]
pub enum WatcherError {
    DeadWatcher,
    Io(std::io::Error),
}

pub struct TextFile {
    text: String,
    last_status: Result<(),std::io::Error>,
    updates: TextResourceInner,
}
impl TextFile {
    pub fn register<P: AsRef<Path>>(path: P) -> Result<TextFile,WatcherError> {
        let (text,inner) = RES_MANAGER.register(path)?;
        Ok(TextFile{
            text,
            last_status: Ok(()),
            updates: inner,
        })
    }

    pub fn get(&mut self) -> &String {
        self.update();
        if let Err(e) = &self.last_status {
            log::warn!("Resource update failed {:?}: {:?}",self.updates.path(),e);
        }
        &self.text
    }
    pub fn strict_get(&mut self) -> Result<&String,WatcherError> {
        self.update();
        match &mut self.last_status {
            Ok(()) => Ok(&self.text),
            Err(_) => {
                let mut tmp = Ok(());
                std::mem::swap(&mut self.last_status,&mut tmp);
                Err(WatcherError::Io(tmp.unwrap_err()))           // safe 
            },
        }        
    }

    fn update(&mut self) {
        if let Some(r) = self.updates.update() {
            self.last_status = match r {
                Ok(t) => {
                    self.text = t;
                    Ok(())
                },
                Err(e) => Err(e),
            };
        }
    }
}


#[cfg(test)]
mod test {

    #[test]
    fn main() {
        
        
        panic!("New lib template")
    }
}

