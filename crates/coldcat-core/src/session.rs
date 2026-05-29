use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ColdCatError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveSession {
    pub id: String,
    pub name: String,
    pub started_at: u64,
    pub end_time: u64,
    pub sites: Vec<String>,
    pub apps: Vec<String>,
}

impl ActiveSession {
    pub fn new(
        name: String,
        duration_secs: u64,
        sites: Vec<String>,
        apps: Vec<String>,
        now: u64,
    ) -> Result<Self> {
        let sites = normalize_sites(sites)?;
        let apps = normalize_apps(apps)?;
        if sites.is_empty() && apps.is_empty() {
            return Err(ColdCatError::InvalidRule(
                "at least one site or app must be provided".to_string(),
            ));
        }

        let end_time = now
            .checked_add(duration_secs)
            .ok_or_else(|| ColdCatError::InvalidDuration(duration_secs.to_string()))?;

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            name,
            started_at: now,
            end_time,
            sites,
            apps,
        })
    }

    pub fn is_active_at(&self, now: u64) -> bool {
        now < self.end_time
    }
}

pub fn now_unix() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ColdCatError::SystemTime)?
        .as_secs())
}

pub fn normalize_sites(sites: Vec<String>) -> Result<Vec<String>> {
    let mut normalized = BTreeSet::new();
    for site in sites {
        let site = normalize_site(&site)?;
        if !site.is_empty() {
            normalized.insert(site);
        }
    }
    Ok(normalized.into_iter().collect())
}

pub fn normalize_apps(apps: Vec<String>) -> Result<Vec<String>> {
    let mut normalized = BTreeSet::new();
    for app in apps {
        let app = app.trim();
        if app.is_empty() {
            continue;
        }
        if app.chars().any(|ch| ch == '\0' || ch == '\n' || ch == '\r') {
            return Err(ColdCatError::InvalidRule(app.to_string()));
        }
        normalized.insert(app.to_string());
    }
    Ok(normalized.into_iter().collect())
}

fn normalize_site(input: &str) -> Result<String> {
    let mut value = input.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Ok(value);
    }

    if let Some(rest) = value.strip_prefix("https://") {
        value = rest.to_string();
    } else if let Some(rest) = value.strip_prefix("http://") {
        value = rest.to_string();
    }

    if let Some((host, _)) = value.split_once('/') {
        value = host.to_string();
    }
    if let Some((host, _)) = value.split_once(':') {
        value = host.to_string();
    }
    value = value.trim_matches('.').to_string();

    if value.is_empty()
        || value
            .chars()
            .any(|ch| ch.is_whitespace() || ch == '\0' || ch == '#')
    {
        return Err(ColdCatError::InvalidRule(input.to_string()));
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_sites() {
        let sites = normalize_sites(vec![
            "https://Reddit.com/r/rust".to_string(),
            "reddit.com".to_string(),
            " old.reddit.com ".to_string(),
        ])
        .unwrap();
        assert_eq!(sites, vec!["old.reddit.com", "reddit.com"]);
    }

    #[test]
    fn rejects_empty_session() {
        assert!(ActiveSession::new("Test".to_string(), 10, vec![], vec![], 100).is_err());
    }
}
