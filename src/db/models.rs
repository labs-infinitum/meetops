#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MeetingListItem {
    pub id: String,
    pub title: String,
    pub meet_url: String,
    pub start_at: i64,
    pub end_at: i64,
    pub auto_join: i64,
}
