use std::fmt;

const PREFIX: &str = "https://meet.google.com/";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MeetUrl(String);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("expected a Google Meet URL like https://meet.google.com/xxx-yyyy-zzz")]
pub struct InvalidMeetUrl;

impl MeetUrl {
    pub fn parse(raw: &str) -> Result<Self, InvalidMeetUrl> {
        let normalized = normalize(raw);
        if normalized.starts_with(PREFIX) && normalized.len() > PREFIX.len() {
            Ok(Self(normalized))
        } else {
            Err(InvalidMeetUrl)
        }
    }

    pub fn ad_hoc_title(&self) -> String {
        let code = self
            .0
            .strip_prefix(PREFIX)
            .unwrap_or("meeting");
        format!("Ad-hoc · {code}")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn normalize(raw: &str) -> String {
    let trimmed = raw.trim();
    let base = trimmed
        .split(&['?', '#'][..])
        .next()
        .unwrap_or(trimmed)
        .trim_end_matches('/');

    if let Some(code) = base.strip_prefix("https://meet.google.com/") {
        if !code.is_empty() {
            return format!("{PREFIX}{code}");
        }
    }
    if let Some(code) = base.strip_prefix("http://meet.google.com/") {
        if !code.is_empty() {
            return format!("{PREFIX}{code}");
        }
    }
    if let Some(code) = base.strip_prefix("meet.google.com/") {
        if !code.is_empty() {
            return format!("{PREFIX}{code}");
        }
    }

    base.to_owned()
}

impl fmt::Display for MeetUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::MeetUrl;

    #[test]
    fn accepts_google_meet_urls() {
        assert!(MeetUrl::parse("https://meet.google.com/abc-defg-hij").is_ok());
    }

    #[test]
    fn rejects_non_meet_urls() {
        assert!(MeetUrl::parse("https://example.com/abc-defg-hij").is_err());
    }

    #[test]
    fn rejects_bare_prefix() {
        assert!(MeetUrl::parse("https://meet.google.com/").is_err());
    }

    #[test]
    fn normalizes_trailing_slash_and_query() {
        let url = MeetUrl::parse("https://meet.google.com/abc-defg-hij/?authuser=0").unwrap();
        assert_eq!(url.as_str(), "https://meet.google.com/abc-defg-hij");
    }

    #[test]
    fn accepts_bare_meet_host() {
        let url = MeetUrl::parse("meet.google.com/abc-defg-hij").unwrap();
        assert_eq!(url.as_str(), "https://meet.google.com/abc-defg-hij");
    }
}
