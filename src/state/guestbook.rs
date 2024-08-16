use color_eyre::eyre::{eyre, Result};
use cuid2::cuid;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct GuestbookEntry {
    pub id: String,
    pub date: u64,
    pub by: String,
    pub message: String,
    pub approved: bool,
}

#[derive(Clone)]
pub struct GuestbookState {}

impl GuestbookState {
    pub fn add_guestbook_entry(&self, entry: &str) -> Result<()> {
        let id = cuid();

        let entry = GuestbookEntry {
            id: id.clone(),
            date: time::OffsetDateTime::now_utc().unix_timestamp() as u64,
            by: "guest".to_string(),
            message: entry.to_string(),
            approved: false,
        };

        self.guestbook.set(&id, &entry)?;
        Ok(())
    }

    pub fn guestbook_entries(&self) -> Result<Vec<GuestbookEntry>> {
        let entries = self.guestbook.iter()?.collect::<Result<Vec<_>, _>>()?;
        Ok(entries.into_iter().map(|(_, v)| v).collect())
    }

    pub fn approved_guestbook_entries(&self) -> Result<Vec<GuestbookEntry>> {
        let entries = self.guestbook.iter()?.collect::<Result<Vec<_>, _>>()?;
        Ok(entries
            .into_iter()
            .map(|(_, v)| v)
            .filter(|entry| entry.approved)
            .collect())
    }

    pub fn approve_guestbook_entry(&self, id: &str) -> Result<()> {
        let entry: GuestbookEntry = self
            .guestbook
            .get(id)?
            .ok_or_else(|| eyre!("entry not found"))?;

        let entry = GuestbookEntry {
            approved: true,
            ..entry
        };

        self.guestbook.set(id, &entry)?;
        Ok(())
    }
}
