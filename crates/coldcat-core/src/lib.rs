pub mod apps;
pub mod duration;
pub mod error;
pub mod hosts;
pub mod protocol;
pub mod session;
pub mod state;

pub use error::{ColdCatError, Result};

pub const DEFAULT_DAEMON_ADDR: &str = "127.0.0.1:47842";
