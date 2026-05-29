use std::fmt;

const PREFIX: &str = "https://meet.google.com/";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MeetUrl(String);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("expected a Google Meet URL like https://meet.google.com/xxx-yyyy-zzz")]
pub struct InvalidMeetUrl;

impl MeetUrl {
    pub fn parse(raw: &str) -> Result<Self, InvalidMeetUrl> {
        if raw.starts_with(PREFIX) && raw.len() > PREFIX.len() {
            Ok(Self(raw.to_owned()))
        } else {
            Err(InvalidMeetUrl)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
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
}
