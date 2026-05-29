use crate::{ColdCatError, Result};

pub fn parse_duration_secs(input: &str) -> Result<u64> {
    let value = input.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Err(ColdCatError::InvalidDuration(input.to_string()));
    }

    if value.chars().all(|ch| ch.is_ascii_digit()) {
        return parse_positive_number(&value, input);
    }

    let mut total = 0_u64;
    let mut number = String::new();
    let mut saw_unit = false;

    for ch in value.chars() {
        if ch.is_ascii_digit() {
            number.push(ch);
            continue;
        }

        if number.is_empty() {
            return Err(ColdCatError::InvalidDuration(input.to_string()));
        }

        let amount = parse_positive_number(&number, input)?;
        let multiplier = match ch {
            's' => 1,
            'm' => 60,
            'h' => 60 * 60,
            'd' => 24 * 60 * 60,
            'w' => 7 * 24 * 60 * 60,
            _ => return Err(ColdCatError::InvalidDuration(input.to_string())),
        };

        total = total
            .checked_add(
                amount
                    .checked_mul(multiplier)
                    .ok_or_else(|| ColdCatError::InvalidDuration(input.to_string()))?,
            )
            .ok_or_else(|| ColdCatError::InvalidDuration(input.to_string()))?;
        number.clear();
        saw_unit = true;
    }

    if !number.is_empty() || !saw_unit || total == 0 {
        return Err(ColdCatError::InvalidDuration(input.to_string()));
    }

    Ok(total)
}

fn parse_positive_number(value: &str, original: &str) -> Result<u64> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| ColdCatError::InvalidDuration(original.to_string()))?;
    if parsed == 0 {
        return Err(ColdCatError::InvalidDuration(original.to_string()));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_units() {
        assert_eq!(parse_duration_secs("30s").unwrap(), 30);
        assert_eq!(parse_duration_secs("15m").unwrap(), 900);
        assert_eq!(parse_duration_secs("2h").unwrap(), 7200);
        assert_eq!(parse_duration_secs("1d").unwrap(), 86400);
    }

    #[test]
    fn parses_compound_duration() {
        assert_eq!(parse_duration_secs("1h30m15s").unwrap(), 5415);
    }

    #[test]
    fn rejects_invalid_duration() {
        assert!(parse_duration_secs("").is_err());
        assert!(parse_duration_secs("0m").is_err());
        assert!(parse_duration_secs("10").is_ok());
        assert!(parse_duration_secs("10x").is_err());
        assert!(parse_duration_secs("1h30").is_err());
    }
}
