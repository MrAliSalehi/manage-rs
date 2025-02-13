use crate::models;
use crate::models::server::Server;
use crate::prelude::Res;
use itertools::Itertools;
use native_db::{Database, Models};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, LazyLock};
use eyre::eyre;

static MODELS: LazyLock<Models> = LazyLock::new(|| {
    let mut models = Models::new();
    models.define::<models::server::Server>().unwrap();
    models
});

#[derive(Clone)]
pub struct DbDriver {
    pub db: Arc<Database<'static>>,
}

impl DbDriver {
    pub(crate) fn new(db_path: &str) -> Self {
        let db_path = PathBuf::from_str(db_path).unwrap();
        log::info!("loading db: {db_path:?}");
        let builder = native_db::Builder::new();
        let db: Database = if db_path.exists() {
            builder.open(&MODELS, db_path.as_path()).unwrap()
        } else {
            builder.create(&MODELS, db_path.as_path()).unwrap()
        };

        Self { db: Arc::new(db) }
    }
    pub fn all_servers(&self) -> eyre::Result<Vec<Server>> {
        let t = self.db.r_transaction()?;

        Ok(t.scan()
            .primary::<Server>()?
            .all()?
            .map(|f| f.unwrap())
            .collect_vec())
    }

    pub fn add_server(&self, server: Server) -> Res {
        let t = self.db.rw_transaction()?;
        t.insert(server)?;
        t.commit()?;
        Ok(())
    }

    pub fn get_server_by_id(&self, id: String) -> eyre::Result<Option<Server>> {
        let r = self.db.r_transaction()?;
        Ok(r.get().primary::<Server>(id)?)
    }

    pub fn update_server(&self, server: Server) -> eyre::Result<Option<Server>> {
        let r = self.db.rw_transaction()?;
        let update = r.auto_update(server)?;
        r.commit()?;
        Ok(update)
    }

    pub fn delete_server(&self, id: String) -> Res {
        let r = self.db.rw_transaction()?;
        let item = r.get().primary::<Server>(id)?.ok_or(eyre!("server not found"))?;
        r.remove(item)?;
        r.commit()?;
        Ok(())
    }
}
