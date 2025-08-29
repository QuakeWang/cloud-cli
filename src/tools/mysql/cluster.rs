use super::parser::{parse_key_value_pairs, split_into_blocks};
use crate::error::Result;
use crate::tools::common::fs_utils;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// Macro definitions for parsing MySQL output fields
macro_rules! parse_string_field {
    ($fields:expr, $key:expr) => {
        $fields.get($key)?.trim().to_string()
    };
}

macro_rules! parse_port_field {
    ($fields:expr, $key:expr) => {
        $fields.get($key)?.trim().parse().ok()?
    };
}

macro_rules! parse_bool_field {
    ($fields:expr, $key:expr) => {
        $fields.get($key)?.trim() == "true"
    };
}

macro_rules! validate_required_field {
    ($item:expr, $field_name:ident, $component_type:expr, $index:expr) => {
        if $item.$field_name.is_empty() {
            return Err(crate::error::CliError::ConfigError(format!(
                "{} {} has an empty {}",
                $component_type,
                $index,
                stringify!($field_name)
            )));
        }
    };
}

/// Represents a Doris Frontend node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frontend {
    pub name: String,
    pub host: String,
    pub edit_log_port: u16,
    pub http_port: u16,
    pub query_port: u16,
    pub rpc_port: u16,
    pub role: String,
    pub is_master: bool,
    pub cluster_id: String,
    pub alive: bool,
    pub version: String,
}

impl Frontend {
    /// Parse a Frontend from a block of MySQL output
    pub fn parse_from_block(block: &str) -> Option<Self> {
        let fields = parse_key_value_pairs(block);

        // Extract required fields using macros
        let name = parse_string_field!(fields, "Name");
        let host = parse_string_field!(fields, "Host");
        let edit_log_port = parse_port_field!(fields, "EditLogPort");
        let http_port = parse_port_field!(fields, "HttpPort");
        let query_port = parse_port_field!(fields, "QueryPort");
        let rpc_port = parse_port_field!(fields, "RpcPort");
        let role = parse_string_field!(fields, "Role");
        let is_master = parse_bool_field!(fields, "IsMaster");
        let cluster_id = parse_string_field!(fields, "ClusterId");
        let alive = parse_bool_field!(fields, "Alive");
        let version = parse_string_field!(fields, "Version");

        Some(Frontend {
            name,
            host,
            edit_log_port,
            http_port,
            query_port,
            rpc_port,
            role,
            is_master,
            cluster_id,
            alive,
            version,
        })
    }
}

/// Represents a Doris Backend node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backend {
    pub backend_id: String,
    pub host: String,
    pub heartbeat_port: u16,
    pub be_port: u16,
    pub http_port: u16,
    pub brpc_port: u16,
    pub alive: bool,
    pub version: String,
    pub status: String,
    pub node_role: String,
    pub tag: Option<String>,
}

impl Backend {
    /// Parse a Backend from a block of MySQL output
    pub fn parse_from_block(block: &str) -> Option<Self> {
        let fields = parse_key_value_pairs(block);

        // Extract required fields using macros
        let backend_id = parse_string_field!(fields, "BackendId");
        let host = parse_string_field!(fields, "Host");
        let heartbeat_port = parse_port_field!(fields, "HeartbeatPort");
        let be_port = parse_port_field!(fields, "BePort");
        let http_port = parse_port_field!(fields, "HttpPort");
        let brpc_port = parse_port_field!(fields, "BrpcPort");
        let alive = parse_bool_field!(fields, "Alive");
        let version = parse_string_field!(fields, "Version");
        let status = parse_string_field!(fields, "Status");
        let node_role = parse_string_field!(fields, "NodeRole");

        // Extract Tag information
        let tag = fields.get("Tag").map(|s| Self::parse_tag_info(s.trim()));

        Some(Backend {
            backend_id,
            host,
            heartbeat_port,
            be_port,
            http_port,
            brpc_port,
            alive,
            version,
            status,
            node_role,
            tag: tag.flatten(),
        })
    }

    /// Parse Tag information and extract cloud cluster information
    fn parse_tag_info(tag_str: &str) -> Option<String> {
        if tag_str.is_empty() || tag_str == "{}" {
            return None;
        }

        // Try to parse JSON format Tag
        match serde_json::from_str::<serde_json::Value>(tag_str) {
            Ok(json) => {
                // Extract key information
                let mut extracted_info = serde_json::Map::new();

                // Support multiple possible field names
                if let Some(cloud_cluster_id) = json.get("cloud_cluster_id") {
                    extracted_info.insert("cloud_cluster_id".to_string(), cloud_cluster_id.clone());
                } else if let Some(cloud_unique_id) = json.get("cloud_unique_id") {
                    extracted_info.insert("cloud_cluster_id".to_string(), cloud_unique_id.clone());
                }

                if let Some(cloud_cluster_name) = json.get("cloud_cluster_name") {
                    extracted_info
                        .insert("cloud_cluster_name".to_string(), cloud_cluster_name.clone());
                } else if let Some(compute_group_name) = json.get("compute_group_name") {
                    extracted_info
                        .insert("cloud_cluster_name".to_string(), compute_group_name.clone());
                }

                if let Some(location) = json.get("location") {
                    extracted_info.insert("location".to_string(), location.clone());
                }

                // If key information is extracted, return simplified JSON
                if !extracted_info.is_empty() {
                    Some(
                        serde_json::to_string(&extracted_info)
                            .unwrap_or_else(|_| tag_str.to_string()),
                    )
                } else {
                    Some(tag_str.to_string())
                }
            }
            Err(_) => Some(tag_str.to_string()),
        }
    }
}

/// Holds information about the entire Doris cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterInfo {
    pub frontends: Vec<Frontend>,
    pub backends: Vec<Backend>,
}

impl ClusterInfo {
    pub fn load_from_file() -> Result<Self> {
        let config_dir = fs_utils::get_user_config_dir()?;
        let file_path = config_dir.join("clusters.toml");
        let content = fs_utils::read_file_content(&file_path)?;
        let info: ClusterInfo = toml::from_str(&content).map_err(|e| {
            crate::error::CliError::ConfigError(format!("Failed to parse clusters.toml: {e}"))
        })?;
        Ok(info)
    }

    pub fn list_be_hosts(&self) -> Vec<String> {
        self.backends
            .iter()
            .filter(|b| b.alive)
            .map(|b| b.host.clone())
            .collect()
    }

    pub fn save_to_file(&self) -> Result<PathBuf> {
        self.validate()?;
        let config_dir = fs_utils::get_user_config_dir()?;
        let file_path = config_dir.join("clusters.toml");
        fs_utils::save_toml_to_file(self, &file_path)?;
        Ok(file_path)
    }

    /// Validates the integrity of the cluster information.
    pub fn validate(&self) -> Result<()> {
        if self.frontends.is_empty() {
            return Err(crate::error::CliError::ConfigError(
                "No frontend nodes found".to_string(),
            ));
        }

        // Validate frontends using macro
        for (i, fe) in self.frontends.iter().enumerate() {
            validate_required_field!(fe, host, "Frontend", i);
            validate_required_field!(fe, cluster_id, "Frontend", i);
            validate_required_field!(fe, version, "Frontend", i);
        }

        // Validate backends using macro
        for (i, be) in self.backends.iter().enumerate() {
            validate_required_field!(be, backend_id, "Backend", i);
            validate_required_field!(be, host, "Backend", i);
            validate_required_field!(be, version, "Backend", i);
        }

        Ok(())
    }

    /// Parse frontends from MySQL output
    pub fn parse_frontends_from_output(output: &str) -> Vec<Frontend> {
        let mut frontends = Vec::new();
        for block in split_into_blocks(output) {
            if let Some(fe) = Frontend::parse_from_block(&block) {
                frontends.push(fe);
            }
        }
        frontends
    }

    /// Parse backends from MySQL output
    pub fn parse_backends_from_output(output: &str) -> Vec<Backend> {
        let mut backends = Vec::new();
        for block in split_into_blocks(output) {
            if let Some(be) = Backend::parse_from_block(&block) {
                backends.push(be);
            }
        }
        backends
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frontend_parse_from_real_output() {
        let block = r#"
*************************** 1. row ***************************
              Name: fe_94c2c212_b35b_4407_aa42_b208f5f63df2
              Host: 192.168.0.1
       EditLogPort: 9010
          HttpPort: 8030
         QueryPort: 9030
           RpcPort: 9020
ArrowFlightSqlPort: -1
              Role: FOLLOWER
          IsMaster: true
         ClusterId: 2133959080
              Join: true
             Alive: true
 ReplayedJournalId: 480298
     LastStartTime: 2025-07-31 18:06:19
     LastHeartbeat: 2025-08-01 14:47:21
          IsHelper: true
            ErrMsg: 
           Version: doris-3.0.2
  CurrentConnected: Yes
"#;

        let frontend = Frontend::parse_from_block(block);
        assert!(frontend.is_some());

        let fe = frontend.unwrap();
        assert_eq!(fe.name, "fe_94c2c212_b35b_4407_aa42_b208f5f63df2");
        assert_eq!(fe.host, "192.168.0.1");
        assert_eq!(fe.edit_log_port, 9010);
        assert_eq!(fe.http_port, 8030);
        assert_eq!(fe.query_port, 9030);
        assert_eq!(fe.rpc_port, 9020);
        assert_eq!(fe.role, "FOLLOWER");
        assert!(fe.is_master);
        assert_eq!(fe.cluster_id, "2133959080");
        assert!(fe.alive);
        assert_eq!(fe.version, "doris-3.0.2");
    }

    #[test]
    fn test_backend_parse_from_real_output() {
        let block = r#"
*************************** 1. row ***************************
              BackendId: 1751558294712
                   Host: 192.168.10.2
          HeartbeatPort: 9050
                 BePort: 9060
               HttpPort: 8040
               BrpcPort: 8060
     ArrowFlightSqlPort: -1
          LastStartTime: 2025-08-01 14:46:17
          LastHeartbeat: 2025-08-01 14:47:11
                  Alive: true
   SystemDecommissioned: false
              TabletNum: 255
       DataUsedCapacity: 6.599 MB
      TrashUsedCapacity: 0.000 
          AvailCapacity: 489.820 GB
          TotalCapacity: 3.437 TB
                UsedPct: 86.08 %
         MaxDiskUsedPct: 86.08 %
     RemoteUsedCapacity: 0.000 
                    Tag: {"location" : "default"}
                 ErrMsg: 
                Version: doris-3.0.2
                 Status: {"lastSuccessReportTabletsTime":"2025-08-01 14:46:22","lastStreamLoadTime":-1,"isQueryDisabled":false,"isLoadDisabled":false,"isActive":true,"currentFragmentNum":0,"lastFragmentUpdateTime":1754030801378}
HeartbeatFailureCounter: 0
               NodeRole: mix
               CpuCores: 96
                 Memory: 375.81 GB
"#;

        let backend = Backend::parse_from_block(block);
        assert!(backend.is_some());

        let be = backend.unwrap();
        assert_eq!(be.backend_id, "1751558294712");
        assert_eq!(be.host, "192.168.10.2");
        assert_eq!(be.heartbeat_port, 9050);
        assert_eq!(be.be_port, 9060);
        assert_eq!(be.http_port, 8040);
        assert_eq!(be.brpc_port, 8060);
        assert!(be.alive);
        assert_eq!(be.version, "doris-3.0.2");
        assert!(be.status.contains("lastSuccessReportTabletsTime"));
        assert_eq!(be.node_role, "mix");
        assert!(be.tag.is_some());
        assert!(be.tag.unwrap().contains("location"));
    }
}
