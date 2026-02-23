mod book_repository;
mod folder_repository;
mod library_repository;
mod tag_repository;

pub use book_repository::{BookRepository, SqliteBookRepository};
pub use folder_repository::{FolderRepository, SqliteFolderRepository};
pub use library_repository::{LibraryRepository, SqliteLibraryRepository};
pub use tag_repository::{SqliteTagRepository, TagRepository};

use crate::error::Result;

pub trait Repository {
    type Entity;
    type Id;

    fn find_by_id(&self, id: &Self::Id) -> Result<Option<Self::Entity>>;
    fn save(&self, entity: &Self::Entity) -> Result<()>;
    fn delete(&self, id: &Self::Id) -> Result<bool>;
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
pub trait AsyncRepository: Send + Sync {
    type Entity: Send + Sync;
    type Id: Send + Sync;

    async fn find_by_id(&self, id: &Self::Id) -> Result<Option<Self::Entity>>;
    async fn save(&self, entity: &Self::Entity) -> Result<()>;
    async fn delete(&self, id: &Self::Id) -> Result<bool>;
}
