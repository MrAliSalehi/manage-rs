use crate::libs::app_config::AppConfigRef;
use crate::libs::db_driver::DbDriver;
use std::sync::Arc;
use crate::libs::agent_service::AgentService;

#[derive(Clone)]
pub struct SharedState {
    //impls deref, can be private
    inner: Arc<SharedStateInner>,
}

#[derive(Clone)]
pub struct SharedStateInner {
    pub app_config: AppConfigRef,
    pub db_driver: DbDriver,
    pub agent_service: AgentService,
}

impl SharedState {
    pub async fn new(config: AppConfigRef, agent_service: AgentService) -> Self {
        Self {
            inner: Arc::new(SharedStateInner {
                agent_service,
                db_driver: DbDriver::new(&config.db_path),
                app_config: config,
            }),
        }
    }
}

impl std::ops::Deref for SharedState {
    type Target = SharedStateInner;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
