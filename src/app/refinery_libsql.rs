use axum::async_trait;
use libsql::{Connection, Error as RqlError};
use refinery::Migration;
use refinery_core::traits::r#async::{AsyncQuery, AsyncTransaction};
use refinery_core::AsyncMigrate;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub(crate) struct LibsqlConn(pub(crate) Connection);

async fn query_applied_migrations(
    transaction: &libsql::Transaction,
    query: &str,
) -> Result<Vec<Migration>, RqlError> {
    let mut stmt = transaction.prepare(query).await?;
    let mut rows = stmt.query(()).await?;
    let mut applied = Vec::new();
    while let Some(row) = rows.next().await? {
        let version = row.get(0)?;
        let applied_on: String = row.get(2)?;
        // Safe to call unwrap, as we stored it in RFC3339 format on the database
        let applied_on = OffsetDateTime::parse(&applied_on, &Rfc3339).unwrap();

        let checksum: String = row.get(3)?;
        applied.push(Migration::applied(
            version,
            row.get(1)?,
            applied_on,
            checksum
                .parse::<u64>()
                .expect("checksum must be a valid u64"),
        ));
    }
    Ok(applied)
}

#[async_trait]
impl AsyncTransaction for LibsqlConn {
    type Error = RqlError;
    async fn execute(&mut self, queries: &[&str]) -> Result<usize, Self::Error> {
        let transaction = self.0.transaction().await?;
        let mut count = 0;
        for query in queries {
            transaction.execute_batch(query).await?;
            count += 1;
        }
        transaction.commit().await?;
        Ok(count)
    }
}

#[async_trait]
impl AsyncQuery<Vec<Migration>> for LibsqlConn {
    async fn query(&mut self, query: &str) -> Result<Vec<Migration>, Self::Error> {
        let transaction = self.0.transaction().await?;
        let applied = query_applied_migrations(&transaction, query).await?;
        transaction.commit().await?;
        Ok(applied)
    }
}

impl AsyncMigrate for LibsqlConn {}
