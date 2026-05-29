use crate::session::normalize_sites;
use crate::{ColdCatError, Result};

pub const BEGIN_MARKER: &str = "# BEGIN COLD-CAT-BLOCK";
pub const END_MARKER: &str = "# END COLD-CAT-BLOCK";

pub fn apply_hosts_block(existing: &str, sites: &[String]) -> Result<String> {
    let mut output = strip_managed_block(existing)?;
    let sites = normalize_sites(sites.to_vec())?;
    if sites.is_empty() {
        return Ok(output);
    }

    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    if !output.is_empty() {
        output.push('\n');
    }

    output.push_str(BEGIN_MARKER);
    output.push('\n');
    for site in sites {
        output.push_str("127.0.0.1 ");
        output.push_str(&site);
        output.push('\n');
        output.push_str("::1 ");
        output.push_str(&site);
        output.push('\n');
    }
    output.push_str(END_MARKER);
    output.push('\n');
    Ok(output)
}

pub fn strip_managed_block(existing: &str) -> Result<String> {
    let mut output = Vec::new();
    let mut in_block = false;
    let mut removed = false;

    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed == BEGIN_MARKER {
            if in_block {
                return Err(ColdCatError::MalformedHostsBlock);
            }
            if output
                .last()
                .map(|line: &&str| line.trim().is_empty())
                .unwrap_or(false)
            {
                output.pop();
            }
            in_block = true;
            removed = true;
            continue;
        }
        if trimmed == END_MARKER {
            if !in_block {
                return Err(ColdCatError::MalformedHostsBlock);
            }
            in_block = false;
            continue;
        }
        if !in_block {
            output.push(line);
        }
    }

    if in_block {
        return Err(ColdCatError::MalformedHostsBlock);
    }

    let mut rendered = output.join("\n");
    if !rendered.is_empty() && (existing.ends_with('\n') || removed) {
        rendered.push('\n');
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_and_strips_block() {
        let base = "127.0.0.1 localhost\n";
        let blocked = apply_hosts_block(base, &["reddit.com".to_string()]).unwrap();
        assert!(blocked.contains(BEGIN_MARKER));
        assert!(blocked.contains("127.0.0.1 reddit.com"));
        assert!(blocked.contains("::1 reddit.com"));

        let stripped = strip_managed_block(&blocked).unwrap();
        assert_eq!(stripped, base);
    }

    #[test]
    fn applying_is_idempotent() {
        let once = apply_hosts_block("", &["reddit.com".to_string()]).unwrap();
        let twice = apply_hosts_block(&once, &["reddit.com".to_string()]).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn malformed_block_is_rejected() {
        assert!(strip_managed_block("# BEGIN COLD-CAT-BLOCK\n127.0.0.1 x\n").is_err());
        assert!(strip_managed_block("# END COLD-CAT-BLOCK\n").is_err());
    }
}
