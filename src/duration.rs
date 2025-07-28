use chrono::{Duration, Utc};
use regex::Regex;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct DurationParser;

impl DurationParser {
    pub fn parse(input: &str) -> crate::Result<Duration> {
        let re = Regex::new(r"(?P<num>\d+)(?P<unit>[yMwdhms])")
            .map_err(|e| crate::DoomsdayError::internal(format!("Regex error: {}", e)))?;

        let mut total = Duration::zero();

        for cap in re.captures_iter(input) {
            let num: i64 = cap["num"].parse().map_err(|e| {
                crate::DoomsdayError::invalid_input(format!("Invalid number: {}", e))
            })?;

            let unit = &cap["unit"];

            let duration = match unit {
                "s" => Duration::seconds(num),
                "m" => Duration::minutes(num),
                "h" => Duration::hours(num),
                "d" => Duration::days(num),
                "w" => Duration::weeks(num),
                "M" => Duration::days(num * 30), // Approximate month
                "y" => Duration::days(num * 365), // Approximate year
                _ => {
                    return Err(crate::DoomsdayError::invalid_input(format!(
                        "Unknown unit: {}",
                        unit
                    )))
                }
            };

            total = total + duration;
        }

        if total == Duration::zero() {
            return Err(crate::DoomsdayError::invalid_input(
                "No valid duration found",
            ));
        }

        Ok(total)
    }

    pub fn format_human(duration: Duration) -> String {
        let mut parts = vec![];
        let mut remaining = duration.num_seconds();

        if remaining < 0 {
            return "expired".to_string();
        }

        let years = remaining / (365 * 24 * 3600);
        if years > 0 {
            parts.push(format!("{}y", years));
            remaining %= 365 * 24 * 3600;
        }

        let days = remaining / (24 * 3600);
        if days > 0 {
            parts.push(format!("{}d", days));
            remaining %= 24 * 3600;
        }

        let hours = remaining / 3600;
        if hours > 0 {
            parts.push(format!("{}h", hours));
            remaining %= 3600;
        }

        let minutes = remaining / 60;
        if minutes > 0 {
            parts.push(format!("{}m", minutes));
            remaining %= 60;
        }

        if remaining > 0 || parts.is_empty() {
            parts.push(format!("{}s", remaining));
        }

        parts.join("")
    }

    pub fn until_expiry(expiry: chrono::DateTime<chrono::Utc>) -> Duration {
        expiry - Utc::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(DurationParser::parse("1y").unwrap(), Duration::days(365));
        assert_eq!(DurationParser::parse("2d").unwrap(), Duration::days(2));
        assert_eq!(DurationParser::parse("3h").unwrap(), Duration::hours(3));
        assert_eq!(DurationParser::parse("4m").unwrap(), Duration::minutes(4));
        assert_eq!(DurationParser::parse("5s").unwrap(), Duration::seconds(5));

        assert_eq!(
            DurationParser::parse("1y2d3h4m5s").unwrap(),
            Duration::days(365)
                + Duration::days(2)
                + Duration::hours(3)
                + Duration::minutes(4)
                + Duration::seconds(5)
        );
    }

    #[test]
    fn test_format_human() {
        assert_eq!(DurationParser::format_human(Duration::days(365)), "1y");
        assert_eq!(DurationParser::format_human(Duration::days(2)), "2d");
        assert_eq!(DurationParser::format_human(Duration::hours(3)), "3h");
        assert_eq!(DurationParser::format_human(Duration::minutes(4)), "4m");
        assert_eq!(DurationParser::format_human(Duration::seconds(5)), "5s");

        assert_eq!(
            DurationParser::format_human(
                Duration::days(365)
                    + Duration::days(2)
                    + Duration::hours(3)
                    + Duration::minutes(4)
                    + Duration::seconds(5)
            ),
            "1y2d3h4m5s"
        );
    }
}
