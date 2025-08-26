use crate::config::Config;
use crate::config_loader;
use crate::tools::ToolRegistry;

pub struct AppState {
    pub config: Config,
    pub doris_config: crate::config_loader::DorisConfig,
    pub registry: ToolRegistry,
    pub background_handle: Option<std::thread::JoinHandle<()>>,
}

impl AppState {
    pub fn new() -> crate::error::Result<Self> {
        let doris_config = config_loader::load_config()?;
        let config = config_loader::to_app_config(doris_config.clone());
        let registry = ToolRegistry::new();

        Ok(Self {
            config,
            doris_config,
            registry,
            background_handle: None,
        })
    }

    pub fn spawn_background_tasks_if_needed(&mut self) {
        let fe_process_exists =
            config_loader::process_detector::get_pid_by_env(config_loader::Environment::FE).is_ok();
        let has_mysql = self.doris_config.mysql.is_some();
        if fe_process_exists && has_mysql {
            self.background_handle =
                Some(crate::core::background_tasks::spawn_cluster_info_collector(
                    self.doris_config.clone(),
                ));
        }
    }

    pub fn update_config(&mut self, new_config: Config) {
        self.config = new_config.clone();
        self.doris_config = self.doris_config.clone().with_app_config(&new_config);
        config_loader::persist_configuration(&self.doris_config);
    }

    pub fn reset_runtime_config(&mut self) {
        self.config = Config::new();
    }

    pub fn cleanup(&mut self) {
        if let Some(handle) = self.background_handle.take() {
            let _ = handle.join();
        }
    }
}
