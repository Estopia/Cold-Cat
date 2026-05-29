use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use coldcat_core::protocol::{CommandRequest, CommandResponse, ServiceState, ServiceStatus};
use coldcat_core::session::{now_unix, ActiveSession};
use coldcat_core::DEFAULT_DAEMON_ADDR;
use coldcat_platform::{PlatformPaths, StateStore, SystemEnforcer};

pub fn run_console() -> Result<()> {
    let shutdown = Arc::new(AtomicBool::new(false));
    let can_stop = Arc::new(AtomicBool::new(true));
    let signal_shutdown = shutdown.clone();
    ctrlc::set_handler(move || {
        signal_shutdown.store(true, Ordering::SeqCst);
    })
    .context("installing Ctrl+C handler")?;
    run_with_shutdown(shutdown, can_stop)
}

pub fn run_with_shutdown(shutdown: Arc<AtomicBool>, can_stop: Arc<AtomicBool>) -> Result<()> {
    let paths = PlatformPaths::detect();
    let enforcer = SystemEnforcer::new(paths.clone());
    enforcer.ensure_layout()?;
    let store = StateStore::open(paths)?;
    let engine = Arc::new(Mutex::new(Engine::load(store, enforcer, can_stop)?));

    let tick_engine = engine.clone();
    let tick_shutdown = shutdown.clone();
    let ticker = thread::spawn(move || {
        while !tick_shutdown.load(Ordering::SeqCst) {
            if let Ok(mut engine) = tick_engine.lock() {
                if let Err(err) = engine.tick() {
                    eprintln!("coldcat-daemon enforcement error: {err:#}");
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
    });

    let listener = TcpListener::bind(DEFAULT_DAEMON_ADDR)
        .with_context(|| format!("binding daemon listener at {DEFAULT_DAEMON_ADDR}"))?;
    listener
        .set_nonblocking(true)
        .context("setting listener nonblocking")?;
    eprintln!("coldcat-daemon listening on {DEFAULT_DAEMON_ADDR}");

    while !shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let engine = engine.clone();
                thread::spawn(move || {
                    if let Err(err) = handle_client(stream, engine) {
                        eprintln!("coldcat-daemon client error: {err:#}");
                    }
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(err) => return Err(err).context("accepting client"),
        }
    }

    ticker
        .join()
        .map_err(|_| anyhow!("enforcement thread panicked"))?;
    Ok(())
}

fn handle_client(stream: TcpStream, engine: Arc<Mutex<Engine>>) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone().context("cloning client stream")?);
    let mut line = String::new();
    reader.read_line(&mut line).context("reading command")?;
    let request: CommandRequest = serde_json::from_str(&line).context("decoding command")?;
    let response = {
        let mut engine = engine.lock().map_err(|_| anyhow!("engine lock poisoned"))?;
        engine.handle(request)
    };

    let mut writer = stream;
    serde_json::to_writer(&mut writer, &response).context("encoding response")?;
    writer.write_all(b"\n").context("writing response")?;
    Ok(())
}

struct Engine {
    store: StateStore,
    enforcer: SystemEnforcer,
    active: Option<ActiveSession>,
    tampered_state: bool,
    can_stop: Arc<AtomicBool>,
}

impl Engine {
    fn load(
        store: StateStore,
        enforcer: SystemEnforcer,
        can_stop: Arc<AtomicBool>,
    ) -> Result<Self> {
        let mut tampered_state = false;
        let active = match store.load() {
            Ok(session) => session,
            Err(err) => {
                eprintln!("coldcat-daemon refused sealed state: {err:#}");
                tampered_state = true;
                None
            }
        };

        let now = now_unix()?;
        can_stop.store(
            active
                .as_ref()
                .map(|session| !session.is_active_at(now))
                .unwrap_or(true),
            Ordering::SeqCst,
        );

        Ok(Self {
            store,
            enforcer,
            active,
            tampered_state,
            can_stop,
        })
    }

    fn handle(&mut self, request: CommandRequest) -> CommandResponse {
        match request {
            CommandRequest::Start {
                duration_secs,
                sites,
                apps,
            } => self.start(duration_secs, sites, apps),
            CommandRequest::Status => CommandResponse::ok("status", Some(self.status())),
            CommandRequest::Recover => self.recover(),
        }
    }

    fn start(
        &mut self,
        duration_secs: u64,
        sites: Vec<String>,
        apps: Vec<String>,
    ) -> CommandResponse {
        if self.tampered_state {
            return CommandResponse::error(
                "sealed state validation failed; refusing to start or clear until administrator recovery",
                Some(self.status()),
            );
        }

        if let Err(err) = self.cleanup_if_expired() {
            return CommandResponse::error(
                format!("failed to clean expired session: {err:#}"),
                Some(self.status()),
            );
        }

        let now = match now_unix() {
            Ok(value) => value,
            Err(err) => {
                return CommandResponse::error(
                    format!("failed to read system time: {err}"),
                    Some(self.status()),
                );
            }
        };

        if self
            .active
            .as_ref()
            .map(|session| session.is_active_at(now))
            .unwrap_or(false)
        {
            return CommandResponse::error(
                "a block session is already active; refusing to overwrite it before expiry",
                Some(self.status()),
            );
        }

        let session = match ActiveSession::new(duration_secs, sites, apps, now) {
            Ok(session) => session,
            Err(err) => return CommandResponse::error(err.to_string(), Some(self.status())),
        };

        if let Err(err) = self.store.save(&session) {
            return CommandResponse::error(
                format!("failed to save sealed state: {err:#}"),
                Some(self.status()),
            );
        }
        self.active = Some(session.clone());
        self.can_stop.store(false, Ordering::SeqCst);

        if let Err(err) = self.enforcer.apply_hosts(&session.sites) {
            return CommandResponse::error(
                format!("failed to apply hosts block: {err:#}"),
                Some(self.status()),
            );
        }
        if let Err(err) = self.enforcer.enforce_apps(&session.apps) {
            return CommandResponse::error(
                format!("failed to enforce app block: {err:#}"),
                Some(self.status()),
            );
        }

        CommandResponse::ok(
            format!(
                "block session active until Unix timestamp {}",
                session.end_time
            ),
            Some(self.status()),
        )
    }

    fn recover(&mut self) -> CommandResponse {
        if self.tampered_state {
            return CommandResponse::error(
                "sealed state validation failed; use documented administrator recovery",
                Some(self.status()),
            );
        }

        let now = match now_unix() {
            Ok(value) => value,
            Err(err) => {
                return CommandResponse::error(
                    format!("failed to read system time: {err}"),
                    Some(self.status()),
                );
            }
        };

        if self
            .active
            .as_ref()
            .map(|session| session.is_active_at(now))
            .unwrap_or(false)
        {
            return CommandResponse::error(
                "recover is refused while an enforced session is still active",
                Some(self.status()),
            );
        }

        match self.cleanup() {
            Ok(()) => CommandResponse::ok("recovery cleanup complete", Some(self.status())),
            Err(err) => CommandResponse::error(
                format!("recovery cleanup failed: {err:#}"),
                Some(self.status()),
            ),
        }
    }

    fn tick(&mut self) -> Result<()> {
        if self.tampered_state {
            return Ok(());
        }

        self.cleanup_if_expired()?;
        if let Some(session) = &self.active {
            self.can_stop.store(false, Ordering::SeqCst);
            self.enforcer.apply_hosts(&session.sites)?;
            self.enforcer.enforce_apps(&session.apps)?;
        }
        Ok(())
    }

    fn cleanup_if_expired(&mut self) -> Result<()> {
        let now = now_unix()?;
        if self
            .active
            .as_ref()
            .map(|session| !session.is_active_at(now))
            .unwrap_or(false)
        {
            self.cleanup()?;
        }
        Ok(())
    }

    fn cleanup(&mut self) -> Result<()> {
        self.can_stop.store(true, Ordering::SeqCst);
        self.enforcer.clear_hosts()?;
        self.store.clear()?;
        self.active = None;
        Ok(())
    }

    fn status(&self) -> ServiceStatus {
        let now = now_unix().unwrap_or(0);
        let state = if self.tampered_state {
            ServiceState::Tampered
        } else if self
            .active
            .as_ref()
            .map(|session| session.is_active_at(now))
            .unwrap_or(false)
        {
            ServiceState::Active
        } else if self.active.is_some() {
            ServiceState::ExpiredNeedsRecovery
        } else {
            ServiceState::Idle
        };

        ServiceStatus {
            state,
            now,
            active_session: self.active.clone(),
            tampered_state: self.tampered_state,
        }
    }
}
