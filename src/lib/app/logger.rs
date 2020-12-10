use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use cursive::{theme, Printer, Vec2, View};
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::append::rolling_file::{
    policy::compound::{
        roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger, CompoundPolicy,
    },
    RollingFileAppender,
};
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::filter::{self, threshold::ThresholdFilter, Filter};
use same_file::is_same_file;

use crate::errors::{Error, Result};

const MEM_LOGS_CAPACITY: usize = 100;
const LOG_MAX_FILE_SIZE: u64 = 100_000;
const LOG_MAX_ARCHIVED_FILES: u32 = 2;

lazy_static! {
    pub static ref LOGS: Mutex<VecDeque<Record>> =
        Mutex::new(VecDeque::with_capacity(MEM_LOGS_CAPACITY));
}

#[derive(Debug, Clone)]
pub struct ModuleFilter {
    path: String,
    level: log::LevelFilter,
}

impl ModuleFilter {
    pub fn new<S: Into<String>>(path: S, level: log::LevelFilter) -> Self {
        ModuleFilter {
            path: path.into(),
            level,
        }
    }
}

impl Filter for ModuleFilter {
    fn filter(&self, record: &log::Record) -> filter::Response {
        if record.level() > self.level {
            if let Some(module_path) = record.module_path() {
                if !module_path.starts_with(&self.path) {
                    return filter::Response::Reject;
                }
            }
        }
        filter::Response::Accept
    }
}

#[derive(Debug)]
pub struct MemoryLogger;

#[derive(Debug)]
pub struct Record {
    pub level: log::Level,
    pub time: chrono::DateTime<chrono::Utc>,
    pub message: String,
}

impl log::Log for MemoryLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let mut logs = LOGS.lock().unwrap();
        if logs.len() == logs.capacity() {
            logs.pop_front();
        }
        logs.push_back(Record {
            level: record.level(),
            time: chrono::Utc::now(),
            message: format!("{}", record.args()),
        });
    }

    fn flush(&self) {}
}

pub struct LogConfig<'a> {
    pub dest_dir: &'a Path,
    pub base_file_name: &'a str,
    pub file_ext: Option<&'a str>,
    pub level: log::LevelFilter,
}

impl<'a> Default for LogConfig<'a> {
    fn default() -> Self {
        LogConfig {
            dest_dir: Path::new(concat!("/tmp/", progname!())),
            base_file_name: progname!(),
            file_ext: Some("log"),
            level: log::LevelFilter::Debug,
        }
    }
}

impl<'a> LogConfig<'a> {
    fn dest(&self) -> PathBuf {
        self.dest_dir
            .join(format!("{}{}", self.base_file_name, self.fmt_file_ext()))
    }

    fn build(&self) -> Result<Config> {
        let dest = self.dest();

        let log_file = RollingFileAppender::builder()
            .encoder(Box::new(PatternEncoder::default()))
            .build(
                dest,
                Box::new(CompoundPolicy::new(
                    Box::new(SizeTrigger::new(LOG_MAX_FILE_SIZE)),
                    Box::new(
                        FixedWindowRoller::builder()
                            .build(
                                self.dest_dir
                                    .join(format!(
                                        "{}.{{}}{}",
                                        self.base_file_name,
                                        self.fmt_file_ext()
                                    ))
                                    .to_str()
                                    .unwrap(),
                                LOG_MAX_ARCHIVED_FILES,
                            )
                            .map_err(|e| Error::Std(e))?,
                    ),
                )),
            )
            .map_err(Error::logger)?;

        let new_appender = || {
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(self.level)))
                .filter(Box::new(ModuleFilter::new(
                    progname!(),
                    log::LevelFilter::Info,
                )))
        };

        let mut builder =
            Config::builder().appender(new_appender().build("log_file", Box::new(log_file)));
        let mut root_builder = Root::builder().appender("log_file");

        #[cfg(debug_assertions)]
        {
            builder = builder.appender(new_appender().build(
                "stderr",
                Box::new(ConsoleAppender::builder().target(Target::Stderr).build()),
            ));
            root_builder = root_builder.appender("stderr");
            builder = builder.appender(new_appender().build("memory", Box::new(MemoryLogger)));
            root_builder = root_builder.appender("memory");
        }

        builder
            .build(root_builder.build(self.level))
            .map_err(Error::from)
    }

    fn fmt_file_ext(&self) -> String {
        self.file_ext.map(|s| format!(".{}", s)).unwrap_or_default()
    }
}

pub struct Logger {
    dest: PathBuf,
    handle: log4rs::Handle,
}

impl Logger {
    pub fn default() -> Result<Self> {
        let config = LogConfig::default();
        let handle = log4rs::init_config(config.build()?)?;
        Ok(Logger {
            dest: config.dest(),
            handle,
        })
    }

    pub fn set_config(&mut self, cfg: LogConfig) -> Result<()> {
        let prev_dest = self.dest.as_path();
        let dest = cfg.dest();

        // Move past logs to new log file, if different.
        match is_same_file(prev_dest, &dest) {
            Ok(false) | Err(_) => {
                if let Err(err) = fs::copy(prev_dest, &dest) {
                    warn!(
                        "error moving logs from '{}' to new log file '{}': {}",
                        prev_dest.display(),
                        dest.display(),
                        err,
                    );
                }
            }
            _ => {}
        }

        match cfg.build() {
            Ok(config) => {
                let _ = fs::remove_file(prev_dest);
                self.dest = dest;
                self.handle.set_config(config);
                Ok(())
            }
            Err(err) => Err(err),
        }
    }
}

pub struct LogView;

impl View for LogView {
    fn draw(&self, printer: &Printer) {
        let logs = LOGS.lock().unwrap();
        let skipped = logs.len().saturating_sub(printer.size.y);

        for (i, record) in logs.iter().skip(skipped).enumerate() {
            let y = printer.size.y - i - 1;
            printer.print(
                (0, y),
                &format!(
                    "{} | [     ] {}",
                    record.time.with_timezone(&chrono::Local).format("%T%.3f"),
                    record.message
                ),
            );
            let color = match record.level {
                log::Level::Error => theme::BaseColor::Red.dark(),
                log::Level::Warn => theme::BaseColor::Yellow.dark(),
                log::Level::Info => theme::BaseColor::Black.light(),
                log::Level::Debug => theme::BaseColor::Green.dark(),
                log::Level::Trace => theme::BaseColor::Blue.dark(),
            };
            printer.with_color(color.into(), |printer| {
                printer.print((16, y), &format!("{:5}", record.level))
            });
        }
    }

    fn required_size(&mut self, _constraint: Vec2) -> Vec2 {
        let logs = LOGS.lock().unwrap();

        let level_width = 8; // Width of "[ERROR] "
        let time_width = 16; // Width of "23:59:59.123 | "

        let w = logs
            .iter()
            .map(|record| record.message.len() + level_width + time_width)
            .max()
            .unwrap_or(1);
        let h = logs.len();

        Vec2::new(w, h)
    }
}
