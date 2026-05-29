use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRecord {
    pub name: String,
    pub exe: Option<String>,
}

pub fn process_matches_rule(rule: &str, process: &ProcessRecord) -> bool {
    let rule = normalize(rule);
    if rule.is_empty() {
        return false;
    }

    let name = normalize(&process.name);
    let stem = Path::new(&process.name)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(normalize)
        .unwrap_or_else(|| name.clone());

    if name == rule || stem == rule || windows_exe_variant(&rule) == name {
        return true;
    }

    if let Some(exe) = &process.exe {
        let exe_norm = normalize_path(exe);
        if rule.contains('/') || rule.contains('\\') {
            return exe_norm == normalize_path(&rule) || exe_norm.ends_with(&normalize_path(&rule));
        }
        if let Some(file_name) = Path::new(exe).file_name().and_then(|value| value.to_str()) {
            let file_name = normalize(file_name);
            let file_stem = Path::new(exe)
                .file_stem()
                .and_then(|value| value.to_str())
                .map(normalize)
                .unwrap_or_else(|| file_name.clone());
            return file_name == rule
                || file_stem == rule
                || windows_exe_variant(&rule) == file_name;
        }
    }

    false
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn normalize_path(value: &str) -> String {
    normalize(value).replace('\\', "/")
}

fn windows_exe_variant(value: &str) -> String {
    if value.ends_with(".exe") {
        value.to_string()
    } else {
        format!("{value}.exe")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_windows_exe_by_stem() {
        let process = ProcessRecord {
            name: "Discord.exe".to_string(),
            exe: Some("C:\\Users\\me\\AppData\\Local\\Discord\\Discord.exe".to_string()),
        };
        assert!(process_matches_rule("discord", &process));
        assert!(process_matches_rule("Discord.exe", &process));
    }

    #[test]
    fn matches_by_path_suffix() {
        let process = ProcessRecord {
            name: "helper".to_string(),
            exe: Some("/opt/apps/focus-breaker/helper".to_string()),
        };
        assert!(process_matches_rule("/apps/focus-breaker/helper", &process));
    }

    #[test]
    fn rejects_unrelated_process() {
        let process = ProcessRecord {
            name: "code.exe".to_string(),
            exe: None,
        };
        assert!(!process_matches_rule("discord", &process));
    }
}
