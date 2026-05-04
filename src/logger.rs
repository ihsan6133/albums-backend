use std::{io::Write, sync::{Arc, atomic::{AtomicUsize, Ordering}, mpsc::{self, SyncSender, TrySendError}}, thread::{self, JoinHandle}};

use chrono::{DateTime, Utc};
use colored::{ColoredString, Colorize};
use log::{Level, Log};

enum LogMsg {
    Message(String),
    Shutdown,
    Flush(mpsc::Sender<()>)
}

trait ColoredLog {
    fn colored(&self) -> ColoredString;
}

impl ColoredLog for Level {
    fn colored(&self) -> ColoredString {
        match self {
            Level::Error => self.as_str().red().bold(),
            Level::Warn => self.as_str().yellow(),
            Level::Info => self.as_str().green(),
            Level::Debug => self.as_str().blue(),
            Level::Trace => self.as_str().cyan(),
        }
    }
}

impl ColoredLog for DateTime<Utc> {
    fn colored(&self) -> ColoredString {
        self.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string().black()
    }
}

struct Logger {
    tx: SyncSender<LogMsg>,
    drop_count: Arc<AtomicUsize>,
}

impl Drop for Logger {
    fn drop(&mut self) {
        println!("Shutting down logger");
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Trace        
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let timestamp = Utc::now();
            let message = format!("{} {:<5} {}{} {}\n", timestamp.colored(), record.level().colored(), record.target().ansi_color(221), ":".black(), record.args(), );
            print!("{}", message);
            match self.tx.try_send(LogMsg::Message(message)) {
                Err(TrySendError::Full(_)) => {
                    self.drop_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                },
                _ => {}
            }
        }
    }

    fn flush(&self) {
        let (tx, rx) = mpsc::channel();
        if self.tx.send(LogMsg::Flush(tx)).is_ok() {
            let _ = rx.recv();
        };
    }
}


pub struct LoggerHandle {
    tx: SyncSender<LogMsg>,
    handle: Option<thread::JoinHandle<()>>,    
    has_shutdown: bool,
}

impl LoggerHandle {
    pub fn shutdown(&mut self) {
        if !self.has_shutdown {
            let _ = self.tx.send(LogMsg::Shutdown);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
            self.has_shutdown = true;
        }
    }
}

impl Drop for LoggerHandle {
    
    fn drop(&mut self) {
        if !self.has_shutdown {
            let _ = self.tx.send(LogMsg::Shutdown);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }
}

pub fn init<W: Write + Send + 'static>(mut writer: W) -> JoinHandle<()> {
    let (tx, rx) = mpsc::sync_channel::<LogMsg>(2048);
    let drop_count = Arc::new(AtomicUsize::new(0));


    let counter = drop_count.clone();
    let handle = thread::spawn(move || {
        while let Ok(msg) = rx.recv() {

            let drop_count = counter.swap(0, Ordering::Relaxed);
            if drop_count > 0 {
                let timestamp = Utc::now();
                let _ = writer.write_all(format!("{} WARN: Dropped {} log messages due to full channel.\n", timestamp.format("%Y-%m-%dT%H:%M:%S.%3fZ"), drop_count).as_bytes());
            }

            match msg {
                LogMsg::Message(s) => {
                    let _ = writer.write_all(s.as_bytes());
                }
                LogMsg::Shutdown => {
                    let _ = writer.flush();
                    break;
                }
                LogMsg::Flush(responder) => {
                    let _ = writer.flush();
                    let _ = responder.send(());
                }
            }
        }
    });


    let logger = Logger { tx: tx.clone(), drop_count };
    log::set_boxed_logger(Box::new(logger)).expect("Failed to create logger");
    log::set_max_level(log::LevelFilter::Trace);

    handle
    // return LoggerHandle { tx, handle: Some(handle), has_shutdown: false };
}