use std::borrow::Cow;

pub mod apps;
pub mod client;
pub mod duration;
pub mod error;
pub mod hosts;
pub mod protocol;
pub mod session;
pub mod state;

pub use error::{ColdCatError, Result};

pub const DEFAULT_DAEMON_ADDR: &str = "127.0.0.1:47842";
pub const DAEMON_ADDR_ENV: &str = "COLDCAT_DAEMON_ADDR";

pub fn daemon_addr() -> String {
    let override_addr = std::env::var(DAEMON_ADDR_ENV).ok();
    resolve_daemon_addr(override_addr.as_deref()).into_owned()
}

fn resolve_daemon_addr<'a>(override_addr: Option<&'a str>) -> Cow<'a, str> {
    match override_addr.map(str::trim).filter(|value| !value.is_empty()) {
        Some(address) => Cow::Borrowed(address),
        None => Cow::Borrowed(DEFAULT_DAEMON_ADDR),
    }
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn resolve_daemon_addr_prefers_non_blank_override() {
		assert_eq!(resolve_daemon_addr(Some("127.0.0.1:49000")), "127.0.0.1:49000");
	}

	#[test]
	fn resolve_daemon_addr_falls_back_for_blank_override() {
		assert_eq!(resolve_daemon_addr(Some("   \t")), DEFAULT_DAEMON_ADDR);
		assert_eq!(resolve_daemon_addr(None), DEFAULT_DAEMON_ADDR);
	}
}
