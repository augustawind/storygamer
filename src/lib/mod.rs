#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;

macro_rules! progname {
    () => {
        "storygamer"
    };
}

pub mod app;
pub mod errors;
pub mod parser;
pub mod types;
mod utils;
