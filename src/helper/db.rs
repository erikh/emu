use anyhow::{anyhow, Result};
use log::LevelFilter;
use sqlx::{
    sqlite::{Sqlite, SqliteConnectOptions, SqlitePool, SqlitePoolOptions, SqliteRow},
    ConnectOptions, Encode, FromRow, Type, Value,
};
use std::collections::HashMap;
use std::{str::FromStr, time::Duration};

pub struct DB {
    handle: SqlitePool,
}

impl DB {
    pub async fn new(url: String) -> Result<Self> {
        let mut options = SqliteConnectOptions::from_str(&url)?;
        options.log_statements(LevelFilter::Debug);
        options.log_slow_statements(LevelFilter::Warn, Duration::new(3, 0));
        let handle = SqlitePoolOptions::new()
            .max_connections(100)
            .connect_with(options)
            .await?;

        Ok(Self { handle })
    }

    pub fn handle(&self) -> SqlitePool {
        self.handle.clone()
    }
}

#[async_trait::async_trait]
pub trait DBRecord: Sized + for<'a> FromRow<'a, SqliteRow> + Unpin {
    fn table_name() -> String;
    fn primary_key(&self) -> i64;
    fn columns(&self) -> Vec<String>;
    fn columns_typed(&self) -> HashMap<String, String>;
    fn constraints(&self) -> String;
    fn value(
        &self,
        column: String,
    ) -> Result<Option<impl Value<Database = Sqlite> + Encode<'_, Sqlite> + Type<Sqlite> + Send>>;

    async fn create_table(&self, db: &mut DB) -> Result<()> {
        let mut column_expr = String::from(r#""id" integer not null primary key auto_increment"#);
        for (name, def) in self.columns_typed() {
            column_expr += &format!(",\n\"{}\" {}", name, def);
        }

        let constraints = self.constraints();

        sqlx::query(&format!(
            "create table \"{}\" ({}{}{})",
            Self::table_name(),
            column_expr,
            if constraints.is_empty() { "" } else { ", " },
            constraints
        ))
        .execute(&db.handle())
        .await?;
        Ok(())
    }

    async fn create(&self, db: &mut DB) -> Result<i64> {
        let mut columns = String::from("\"id\"");
        let columnlist = self.columns();

        for column in &columnlist {
            columns += &format!(", \"{}\"", column);
        }

        let mut binds = String::from("?,").repeat(self.columns().len() + 1); // extra for id
        binds.truncate(binds.len() - 2); // remove trailing comma

        let stmt = format!(
            "insert into \"{}\" ({}) values ({})",
            Self::table_name(),
            columns,
            binds,
        );

        let mut query = sqlx::query(&stmt);
        for column in columnlist {
            query = query.bind(self.value(column)?)
        }

        let res = query.execute(&db.handle()).await?;
        Ok(res.last_insert_rowid())
    }

    async fn delete(&self, db: &mut DB) -> Result<()> {
        let res = sqlx::query(&format!(
            "delete from \"{}\" where id = ?",
            Self::table_name()
        ))
        .bind(self.primary_key())
        .execute(&db.handle())
        .await?;

        if res.rows_affected() > 0 {
            Ok(())
        } else {
            Err(anyhow!("No rows found"))
        }
    }

    async fn save(&self, db: &mut DB) -> Result<()> {
        let mut columns = String::new();
        let columnlist = self.columns();

        for column in &columnlist {
            columns += &format!("{}=?, ", column);
        }

        columns.truncate(columns.len() - 3);

        let stmt = format!(
            "update \"{}\" set {} where id = ?",
            columns,
            Self::table_name()
        );

        let mut query = sqlx::query(&stmt);
        for column in columnlist {
            query = query.bind(self.value(column)?)
        }
        let res = query.execute(&db.handle()).await?;

        if res.rows_affected() > 0 {
            Ok(())
        } else {
            Err(anyhow!("No rows found"))
        }
    }

    async fn exists(&self, db: &mut DB) -> Result<bool> {
        Ok(sqlx::query(&format!(
            "select 1 from \"{}\" where id = ?",
            Self::table_name()
        ))
        .bind(self.primary_key())
        .fetch_one(&db.handle())
        .await
        .map_or_else(|_| false, |_| true))
    }

    async fn load_one(db: &mut DB, key: i64) -> Result<Self> {
        Ok(sqlx::query_as(&format!(
            "select * from \"{}\" where id = ?",
            Self::table_name()
        ))
        .bind(key)
        .fetch_one(&db.handle())
        .await?)
    }

    async fn load_limit(
        db: &mut DB,
        key: i64,
        limit: usize,
        offset: Option<usize>,
    ) -> Result<Vec<Self>> {
        Ok(sqlx::query_as(&format!(
            "select * from \"{}\" where id = ? limit {}{}",
            Self::table_name(),
            limit,
            if let Some(offset) = offset {
                format!("offset {}", offset)
            } else {
                Default::default()
            }
        ))
        .bind(key)
        .fetch_all(&db.handle())
        .await?)
    }

    async fn load_all(db: &mut DB, key: i64) -> Result<Vec<Self>> {
        Ok(sqlx::query_as(&format!(
            "select * from \"{}\" where id = ?",
            Self::table_name()
        ))
        .bind(key)
        .fetch_all(&db.handle())
        .await?)
    }
}
