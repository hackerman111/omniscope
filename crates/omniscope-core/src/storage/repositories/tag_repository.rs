use rusqlite::Connection;
use std::sync::MutexGuard;

use crate::error::Result;
use crate::models::Tag;

use super::Repository;

pub trait TagRepository: Repository<Entity = Tag, Id = String> {
    fn list(&self) -> Result<Vec<(String, u32)>>;
    fn find_by_prefix(&self, prefix: &str, limit: usize) -> Result<Vec<String>>;
}

pub struct SqliteTagRepository<'a> {
    conn: MutexGuard<'a, Connection>,
}

impl<'a> SqliteTagRepository<'a> {
    pub fn new(conn: MutexGuard<'a, Connection>) -> Self {
        Self { conn }
    }
}

impl<'a> Repository for SqliteTagRepository<'a> {
    type Entity = Tag;
    type Id = String;

    fn find_by_id(&self, _id: &Self::Id) -> Result<Option<Self::Entity>> {
        let mut stmt = self.conn.prepare("SELECT tags FROM books")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

        for row in rows {
            let tags_str = row?;
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(&tags_str)
                && let Some(tag) = tags.into_iter().next() {
                    return Ok(Some(Tag::new(tag)));
                }
        }
        Ok(None)
    }

    fn save(&self, _entity: &Self::Entity) -> Result<()> {
        Ok(())
    }

    fn delete(&self, _id: &Self::Id) -> Result<bool> {
        Ok(false)
    }
}

impl<'a> TagRepository for SqliteTagRepository<'a> {
    fn list(&self) -> Result<Vec<(String, u32)>> {
        let mut stmt = self.conn.prepare("SELECT tags FROM books")?;
        let mut tag_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            let tags_str = row?;
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(&tags_str) {
                for tag in tags {
                    *tag_counts.entry(tag).or_insert(0) += 1;
                }
            }
        }

        let mut result: Vec<(String, u32)> = tag_counts.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        Ok(result)
    }

    fn find_by_prefix(&self, prefix: &str, limit: usize) -> Result<Vec<String>> {
        let all_tags = self.list()?;
        let prefix_lower = prefix.to_lowercase();
        let matches: Vec<String> = all_tags
            .into_iter()
            .filter(|(tag, _)| tag.to_lowercase().starts_with(&prefix_lower))
            .take(limit)
            .map(|(tag, _)| tag)
            .collect();
        Ok(matches)
    }
}

#[cfg(feature = "async")]
pub mod async_impl {
    use async_trait::async_trait;
    use rusqlite::Connection;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    use crate::error::Result;
    use crate::models::Tag;

    use super::TagRepository;

    pub struct AsyncTagRepository {
        conn: Arc<Mutex<Connection>>,
    }

    impl AsyncTagRepository {
        pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
            Self { conn }
        }
    }

    #[async_trait]
    impl super::super::AsyncRepository for AsyncTagRepository {
        type Entity = Tag;
        type Id = String;

        async fn find_by_id(&self, id: &Self::Id) -> Result<Option<Self::Entity>> {
            let conn = self.conn.lock().await;
            let repo = super::SqliteTagRepository::new(conn);
            repo.find_by_id(id)
        }

        async fn save(&self, entity: &Self::Entity) -> Result<()> {
            let conn = self.conn.lock().await;
            let repo = super::SqliteTagRepository::new(conn);
            repo.save(entity)
        }

        async fn delete(&self, id: &Self::Id) -> Result<bool> {
            let conn = self.conn.lock().await;
            let repo = super::SqliteTagRepository::new(conn);
            repo.delete(id)
        }
    }
}
