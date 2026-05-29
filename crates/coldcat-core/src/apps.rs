use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRecord {
    pub name: String,
    pub exe: Option<String>,
    pub cmdline: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopApp {
    pub name: String,
    pub desktop_id: String,
    pub rule: String,
    pub detail: String,
    pub aliases: Vec<String>,
}

pub fn process_matches_rule(rule: &str, process: &ProcessRecord) -> bool {
    let rule = normalize(rule);
    if rule.is_empty() {
        return false;
    }

    process_aliases(process)
        .iter()
        .any(|alias| alias_matches_rule(alias, &rule))
}

pub fn desktop_app_from_content(path: &Path, content: &str) -> Option<DesktopApp> {
    let desktop_id = path.file_name()?.to_str()?.trim_end_matches(".desktop");
    let mut in_desktop_entry = false;
    let mut name = None;
    let mut hidden = false;
    let mut no_display = false;
    let mut app_type = None;
    let mut exec = None;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Name" => name = Some(value.to_string()),
            "Type" => app_type = Some(value.to_string()),
            "Exec" => exec = Some(value.to_string()),
            "Hidden" => hidden = value.eq_ignore_ascii_case("true"),
            "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }

    if hidden
        || no_display
        || app_type
            .as_deref()
            .is_some_and(|kind| kind != "Application")
    {
        return None;
    }

    let exec = exec?;
    let tokens = split_desktop_exec(&exec);
    let command = desktop_command(&tokens)?;
    let name = name.unwrap_or_else(|| desktop_id.to_string());

    let mut aliases = BTreeSet::new();
    insert_alias(&mut aliases, desktop_id);
    insert_alias(&mut aliases, &format!("{desktop_id}.desktop"));
    insert_alias(&mut aliases, command);

    if let Some(file_name) = executable_file_name(command) {
        insert_alias(&mut aliases, &file_name);
        if let Some(stem) = executable_file_stem(&file_name) {
            insert_alias(&mut aliases, &stem);
        }
    }

    let (rule, source) = if let Some(flatpak_id) = flatpak_app_id(&tokens, desktop_id) {
        insert_alias(&mut aliases, &flatpak_id);
        if let Some(command_alias) = flatpak_command_alias(&tokens) {
            insert_alias(&mut aliases, &command_alias);
        }
        (flatpak_id, "Flatpak")
    } else if let Some(snap_name) = snap_app_name(&tokens, command, desktop_id) {
        insert_alias(&mut aliases, &snap_name);
        (snap_name, "Snap")
    } else {
        let rule = executable_file_name(command).unwrap_or_else(|| command.to_string());
        (rule, "Desktop")
    };

    insert_alias(&mut aliases, &rule);

    Some(DesktopApp {
        name,
        desktop_id: desktop_id.to_string(),
        rule,
        detail: format!("{source}: {} ({exec})", path.display()),
        aliases: aliases.into_iter().collect(),
    })
}

pub fn desktop_app_matches_query(app: &DesktopApp, query: &str) -> bool {
    let query = normalize(query);
    if query.is_empty() {
        return false;
    }

    normalize(&app.name) == query
        || normalize(&app.desktop_id) == query
        || normalize(&app.rule) == query
        || app.aliases.iter().any(|alias| normalize(alias) == query)
}

pub fn split_desktop_exec(exec: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    let mut escaped = false;

    for ch in exec.chars() {
        if escaped {
            token.push(ch);
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        if let Some(quote_char) = quote {
            if ch == quote_char {
                quote = None;
            } else {
                token.push(ch);
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            quote = Some(ch);
        } else if ch.is_whitespace() {
            push_desktop_exec_token(&mut tokens, &mut token);
        } else {
            token.push(ch);
        }
    }

    if escaped {
        token.push('\\');
    }
    push_desktop_exec_token(&mut tokens, &mut token);
    tokens
}

pub fn normalize_app_rule(rule: &str) -> String {
    normalize(rule)
}

fn desktop_command(tokens: &[String]) -> Option<&str> {
    let mut index = 0;
    if tokens
        .first()
        .is_some_and(|token| executable_stem(token) == "env")
    {
        index += 1;
        while let Some(token) = tokens.get(index) {
            if token.starts_with('-') {
                index += 1;
                if matches!(token.as_str(), "-u" | "--unset" | "-S" | "--split-string") {
                    index += 1;
                }
            } else if is_env_assignment(token) {
                index += 1;
            } else {
                break;
            }
        }
    }

    tokens.get(index).map(String::as_str)
}

fn is_env_assignment(token: &str) -> bool {
    token.split_once('=').is_some_and(|(key, _)| {
        !key.is_empty()
            && key
                .chars()
                .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    })
}

fn push_desktop_exec_token(tokens: &mut Vec<String>, token: &mut String) {
    if let Some(cleaned) = clean_desktop_exec_token(token) {
        tokens.push(cleaned);
    }
    token.clear();
}

fn clean_desktop_exec_token(token: &str) -> Option<String> {
    let mut cleaned = String::new();
    let mut chars = token.trim().chars();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            match chars.next() {
                Some('%') => cleaned.push('%'),
                Some(code) if is_desktop_field_code(code) => {}
                Some(code) => {
                    cleaned.push('%');
                    cleaned.push(code);
                }
                None => cleaned.push('%'),
            }
        } else {
            cleaned.push(ch);
        }
    }

    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

fn is_desktop_field_code(code: char) -> bool {
    matches!(
        code,
        'f' | 'F' | 'u' | 'U' | 'd' | 'D' | 'n' | 'N' | 'i' | 'c' | 'k' | 'v' | 'm'
    )
}

fn flatpak_app_id(tokens: &[String], desktop_id: &str) -> Option<String> {
    let run_index = tokens
        .iter()
        .position(|token| executable_stem(token) == "flatpak")?;
    if tokens.get(run_index + 1).map(String::as_str) != Some("run") {
        return None;
    }

    tokens
        .iter()
        .skip(run_index + 2)
        .find(|token| flatpak_token_looks_like_app_id(token))
        .cloned()
        .or_else(|| {
            if desktop_id.contains('.') {
                Some(desktop_id.to_string())
            } else {
                None
            }
        })
}

fn flatpak_command_alias(tokens: &[String]) -> Option<String> {
    tokens
        .iter()
        .find_map(|token| token.strip_prefix("--command=").map(ToString::to_string))
}

fn snap_app_name(tokens: &[String], command: &str, desktop_id: &str) -> Option<String> {
    if executable_stem(command) == "snap" && tokens.get(1).map(String::as_str) == Some("run") {
        return tokens
            .get(2)
            .map(|token| token.split('.').next().unwrap_or(token).to_string());
    }

    let normalized_command = normalize_path(command);
    if let Some(name) = normalized_command.strip_prefix("/snap/bin/") {
        return Some(
            name.split('/')
                .next()
                .unwrap_or(name)
                .split('.')
                .next()
                .unwrap_or(name)
                .to_string(),
        );
    }

    let components: Vec<&str> = normalized_command.split('/').collect();
    components
        .windows(2)
        .find_map(|pair| (pair[0] == "snap").then(|| pair[1].to_string()))
        .or_else(|| {
            desktop_id.contains('_').then(|| {
                desktop_id
                    .split('_')
                    .next()
                    .unwrap_or(desktop_id)
                    .to_string()
            })
        })
}

fn flatpak_token_looks_like_app_id(token: &str) -> bool {
    !token.starts_with('-')
        && !token.starts_with("@@")
        && !is_env_assignment(token)
        && token.split('.').filter(|part| !part.is_empty()).count() >= 3
}

fn process_aliases(process: &ProcessRecord) -> Vec<String> {
    let mut aliases = BTreeSet::new();
    insert_alias(&mut aliases, &process.name);
    insert_process_path_aliases(&mut aliases, &process.name);

    if let Some(exe) = &process.exe {
        insert_alias(&mut aliases, exe);
        insert_process_path_aliases(&mut aliases, exe);
    }

    for token in &process.cmdline {
        insert_alias(&mut aliases, token);
        insert_process_path_aliases(&mut aliases, token);
    }

    aliases.into_iter().collect()
}

fn alias_matches_rule(alias: &str, rule: &str) -> bool {
    let alias = normalize(alias);
    if alias.is_empty() {
        return false;
    }

    if alias == rule || windows_exe_variant(rule) == alias {
        return true;
    }

    if rule.contains('/') || rule.contains('\\') {
        let alias_path = normalize_path(&alias);
        let rule_path = normalize_path(rule);
        return alias_path == rule_path
            || alias_path.ends_with(&format!("/{rule_path}"))
            || alias_path.ends_with(rule_path.trim_start_matches('/'));
    }

    if let Some(file_name) = executable_file_name(&alias) {
        if normalize(&file_name) == rule || windows_exe_variant(rule) == normalize(&file_name) {
            return true;
        }
        if let Some(file_stem) = executable_file_stem(&file_name) {
            if normalize(&file_stem) == rule {
                return true;
            }
        }
    }

    normalize_path(&alias)
        .split('/')
        .filter(|part| !part.is_empty())
        .any(|part| part == rule || windows_exe_variant(rule) == part)
}

fn insert_process_path_aliases(aliases: &mut BTreeSet<String>, value: &str) {
    let normalized = normalize_path(value);
    if let Some(file_name) = executable_file_name(&normalized) {
        insert_alias(aliases, &file_name);
        if let Some(stem) = executable_file_stem(&file_name) {
            insert_alias(aliases, &stem);
        }
    }
    for component in normalized.split('/').filter(|part| !part.is_empty()) {
        insert_alias(aliases, component);
    }
}

fn insert_alias(aliases: &mut BTreeSet<String>, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        aliases.insert(value.to_string());
    }
}

fn executable_file_name(command: &str) -> Option<String> {
    Path::new(command)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn executable_file_stem(file_name: &str) -> Option<String> {
    Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty() && *value != file_name)
        .map(ToString::to_string)
}

fn executable_stem(command: &str) -> String {
    executable_file_stem(&executable_file_name(command).unwrap_or_else(|| command.to_string()))
        .unwrap_or_else(|| executable_file_name(command).unwrap_or_else(|| command.to_string()))
        .to_ascii_lowercase()
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

    fn process(name: &str, exe: Option<&str>, cmdline: &[&str]) -> ProcessRecord {
        ProcessRecord {
            name: name.to_string(),
            exe: exe.map(ToString::to_string),
            cmdline: cmdline.iter().map(|value| value.to_string()).collect(),
        }
    }

    #[test]
    fn matches_windows_exe_by_stem() {
        let process = process(
            "Discord.exe",
            Some("C:\\Users\\me\\AppData\\Local\\Discord\\Discord.exe"),
            &[],
        );
        assert!(process_matches_rule("discord", &process));
        assert!(process_matches_rule("Discord.exe", &process));
    }

    #[test]
    fn matches_by_path_suffix() {
        let process = process("helper", Some("/opt/apps/focus-breaker/helper"), &[]);
        assert!(process_matches_rule("/apps/focus-breaker/helper", &process));
    }

    #[test]
    fn matches_snap_and_flatpak_processes() {
        let snap = process(
            "spotify",
            Some("/snap/spotify/current/usr/share/spotify/spotify"),
            &["/snap/spotify/current/usr/share/spotify/spotify"],
        );
        assert!(process_matches_rule("spotify", &snap));

        let flatpak = process(
            "firefox",
            Some("/usr/bin/flatpak"),
            &["flatpak", "run", "org.mozilla.firefox"],
        );
        assert!(process_matches_rule("org.mozilla.firefox", &flatpak));
    }

    #[test]
    fn matches_command_line_alternate_executable_names() {
        let process = process(
            "python3",
            None,
            &["python3", "/opt/apps/focus-breaker/main.py"],
        );
        assert!(process_matches_rule("focus-breaker", &process));
        assert!(process_matches_rule(
            "/opt/apps/focus-breaker/main.py",
            &process
        ));
    }

    #[test]
    fn rejects_unrelated_process() {
        let process = process("code.exe", None, &[]);
        assert!(!process_matches_rule("discord", &process));
    }

    #[test]
    fn parses_desktop_exec_quoting_and_field_codes() {
        assert_eq!(
            split_desktop_exec(r#""/opt/My App/app" --open %U --name "Cold Cat""#),
            vec!["/opt/My App/app", "--open", "--name", "Cold Cat"]
        );
    }

    #[test]
    fn parses_regular_desktop_file() {
        let app = desktop_app_from_content(
            Path::new("/usr/share/applications/org.gnome.Terminal.desktop"),
            "[Desktop Entry]\nType=Application\nName=Terminal\nExec=/usr/bin/gnome-terminal -- %F\n",
        )
        .unwrap();

        assert_eq!(app.name, "Terminal");
        assert_eq!(app.rule, "gnome-terminal");
        assert!(app.detail.starts_with("Desktop:"));
        assert!(desktop_app_matches_query(&app, "Terminal"));
        assert!(desktop_app_matches_query(
            &app,
            "org.gnome.Terminal.desktop"
        ));
    }

    #[test]
    fn parses_snap_and_flatpak_desktop_files() {
        let snap = desktop_app_from_content(
            Path::new("/var/lib/snapd/desktop/applications/spotify_spotify.desktop"),
            "[Desktop Entry]\nType=Application\nName=Spotify\nExec=env BAMF_DESKTOP_FILE_HINT=/var/lib/snapd/desktop/applications/spotify_spotify.desktop /snap/bin/spotify %U\n",
        )
        .unwrap();
        assert_eq!(snap.rule, "spotify");
        assert!(snap.detail.starts_with("Snap:"));

        let flatpak = desktop_app_from_content(
            Path::new("/var/lib/flatpak/exports/share/applications/org.mozilla.firefox.desktop"),
            "[Desktop Entry]\nType=Application\nName=Firefox\nExec=/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=firefox --file-forwarding org.mozilla.firefox @@u %u @@\n",
        )
        .unwrap();
        assert_eq!(flatpak.rule, "org.mozilla.firefox");
        assert!(flatpak.detail.starts_with("Flatpak:"));
    }

    #[test]
    fn skips_hidden_or_no_display_desktop_files() {
        assert!(desktop_app_from_content(
            Path::new("/usr/share/applications/helper.desktop"),
            "[Desktop Entry]\nType=Application\nName=Helper\nNoDisplay=true\nExec=helper\n",
        )
        .is_none());
    }
}
