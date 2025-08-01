use super::cluster::{Backend, ClusterInfo, Frontend};
use std::collections::HashMap;

/// Parse frontends from MySQL output
pub fn parse_frontends(output: &str) -> Vec<Frontend> {
    ClusterInfo::parse_frontends_from_output(output)
}

/// Parse backends from MySQL output
pub fn parse_backends(output: &str) -> Vec<Backend> {
    ClusterInfo::parse_backends_from_output(output)
}

/// Split MySQL SHOW command output into individual row blocks
pub fn split_into_blocks(output: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current_block = String::new();

    for line in output.lines() {
        // Check if this line is a row separator
        if line.contains("***************************") && line.contains("row") {
            if !current_block.trim().is_empty() {
                blocks.push(current_block.clone());
            }
            current_block.clear();
        }
        
        current_block.push_str(line);
        current_block.push('\n');
    }

    if !current_block.trim().is_empty() {
        blocks.push(current_block);
    }

    blocks
}

/// Parse key-value pairs from a block of text
pub fn parse_key_value_pairs(block: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();

    for line in block.lines() {
        let line = line.trim();
        if line.is_empty() || line.contains("***************************") {
            continue;
        }

        if let Some((key, value)) = parse_key_value(line) {
            fields.insert(key, value);
        }
    }

    fields
}

/// Parse a single key-value line
pub fn parse_key_value(line: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = line.splitn(2, ':').collect();
    if parts.len() == 2 {
        let key = parts[0].trim().to_string();
        let value = parts[1].trim().to_string();
        if !key.is_empty() {
            return Some((key, value));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_into_blocks() {
        let output = r#"
*************************** 1. row ***************************
              Name: fe_94c2c212_b35b_4407_aa42_b208f5f63df2
              Host: 192.168.0.1
       EditLogPort: 9010
          HttpPort: 8030
         QueryPort: 9030
           RpcPort: 9020
              Role: FOLLOWER
          IsMaster: true
         ClusterId: 2133959080
             Alive: true
           Version: doris-3.0.2
*************************** 2. row ***************************
              Name: fe_another_node
              Host: 192.168.0.2
       EditLogPort: 9010
          HttpPort: 8030
         QueryPort: 9030
           RpcPort: 9020
              Role: OBSERVER
          IsMaster: false
         ClusterId: 2133959080
             Alive: true
           Version: doris-3.0.2
"#;

        let blocks = split_into_blocks(output);
        assert_eq!(blocks.len(), 2);

        // Check first block contains expected content
        assert!(blocks[0].contains("fe_94c2c212_b35b_4407_aa42_b208f5f63df2"));
        assert!(blocks[0].contains("192.168.0.1"));
        assert!(blocks[0].contains("FOLLOWER"));

        // Check second block contains expected content
        assert!(blocks[1].contains("fe_another_node"));
        assert!(blocks[1].contains("192.168.0.2"));
        assert!(blocks[1].contains("OBSERVER"));
    }
}
