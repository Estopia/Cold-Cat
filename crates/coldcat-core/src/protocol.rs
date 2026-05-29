use serde::{Deserialize, Serialize};

use crate::session::ActiveSession;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandRequest {
    Start {
        name: String,
        duration_secs: u64,
        sites: Vec<String>,
        apps: Vec<String>,
    },
    Status,
    Recover,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandResponse {
    pub ok: bool,
    pub message: String,
    pub status: Option<ServiceStatus>,
}

impl CommandResponse {
    pub fn ok(message: impl Into<String>, status: Option<ServiceStatus>) -> Self {
        Self {
            ok: true,
            message: message.into(),
            status,
        }
    }

    pub fn error(message: impl Into<String>, status: Option<ServiceStatus>) -> Self {
        Self {
            ok: false,
            message: message.into(),
            status,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceStatus {
    pub state: ServiceState,
    pub now: u64,
    pub active_sessions: Vec<ActiveSession>,
    pub tampered_state: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    Idle,
    Active,
    ExpiredNeedsRecovery,
    Tampered,
}
