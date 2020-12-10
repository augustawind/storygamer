pub mod core;
pub mod logger;
pub mod ui;

pub use self::core::Game;
use self::logger::Logger;
use crate::errors::Result;
pub use ui::run;

// Container that holds all of the dynamic application state.
pub struct AppState {
    pub game: Option<Game>,
    pub logger: Logger,
}

impl AppState {
    pub fn new() -> Result<Self> {
        log_panics::init();
        Ok(AppState {
            game: None,
            logger: Logger::default()?,
        })
    }
}
