#![allow(dead_code)]
use super::*;
use crate::network::*;
use anyhow::anyhow;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, sqlx::FromRow)]
pub struct DBNetwork {
    id: i64,
    #[sqlx(flatten)]
    record: Network,
}

impl DBNetwork {
    fn new(name: String) -> Self {
        Self {
            id: 0,
            record: Network { name },
        }
    }
}

#[async_trait::async_trait]
impl DBRecord for DBNetwork
where
    Self: Sized + Unpin,
{
    fn table_name() -> &'static str {
        "networks"
    }

    fn set_primary_key(&mut self, id: i64) {
        self.id = id
    }

    fn primary_key(&self) -> i64 {
        self.id
    }

    fn columns(&self) -> Vec<&str> {
        vec!["name"]
    }

    fn columns_typed(&self) -> HashMap<&str, &str> {
        let mut map = HashMap::default();
        map.insert("name", "varchar not null");
        map
    }

    fn constraints(&self) -> &str {
        ""
    }

    fn value(&self, column: &str) -> Result<impl Type<Sqlite> + Encode<'_, Sqlite> + Send> {
        if column == "name" {
            return Ok(self.record.name.clone());
        }

        Err(anyhow!("not a column"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_db_networks() -> Result<()> {
        let tf = NamedTempFile::new()?;
        let path = tf.into_temp_path();

        let mut db = DB::new(format!("sqlite://{}", path.to_str().unwrap())).await?;
        let mut network = DBNetwork::new("foo".to_string());

        assert_eq!(DBNetwork::table_name(), "networks");
        network.create_table(&mut db).await?;
        network.create(&mut db).await?;

        let network2 = DBNetwork::load_one(&mut db, network.id).await?;

        assert_eq!(network, network2);

        let mut network3 = DBNetwork::new("bar".to_string());
        network3.create(&mut db).await?;
        let networks = DBNetwork::load_all(&mut db).await?;

        assert_eq!(networks, vec![network, network3]);
        Ok(())
    }
}
