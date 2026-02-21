use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use super::schema::apply_pragmas;
use crate::error::Result;

pub struct ConnectionPool {
    path: Option<String>,
    connection: Mutex<Connection>,
}

impl ConnectionPool {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        apply_pragmas(&conn)?;
        Ok(Self {
            path: Some(path.to_string_lossy().to_string()),
            connection: Mutex::new(conn),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        apply_pragmas(&conn)?;
        Ok(Self {
            path: None,
            connection: Mutex::new(conn),
        })
    }

    pub fn get_connection(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.connection.lock().unwrap()
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub fn is_in_memory(&self) -> bool {
        self.path.is_none()
    }
}

#[cfg(feature = "async")]
pub mod async_pool {
    use rusqlite::Connection;
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    use super::super::schema::apply_pragmas;
    use crate::error::Result;

    pub struct AsyncConnectionPool {
        path: Option<String>,
        connection: Arc<Mutex<Connection>>,
    }

    impl AsyncConnectionPool {
        pub async fn open(path: &Path) -> Result<Self> {
            let conn = Connection::open(path)?;
            apply_pragmas(&conn)?;
            Ok(Self {
                path: Some(path.to_string_lossy().to_string()),
                connection: Arc::new(Mutex::new(conn)),
            })
        }

        pub async fn open_in_memory() -> Result<Self> {
            let conn = Connection::open_in_memory()?;
            apply_pragmas(&conn)?;
            Ok(Self {
                path: None,
                connection: Arc::new(Mutex::new(conn)),
            })
        }

        pub async fn get_connection(&self) -> tokio::sync::MutexGuard<'_, Connection> {
            self.connection.lock().await
        }
    }
}
