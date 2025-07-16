use crate::config::Config;
use crate::error::{CliError, Result};
use crate::executor;
use crate::tools::{ExecutionResult, Tool};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub struct PstackTool;

impl Tool for PstackTool {
    fn name(&self) -> &str {
        "pstack"
    }

    fn description(&self) -> &str {
        "Generate process stack trace (.txt)"
    }

    fn execute(&self, config: &Config, pid: u32) -> Result<ExecutionResult> {
        config.ensure_output_dir()?;

        let script_path = PathBuf::from("/opt/selectdb/ps.sh");
        self.ensure_pstack_script(&script_path)?;

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("pstack_{pid}_{timestamp}.txt");
        let output_path = config.output_dir.join(filename);

        let mut command = Command::new("bash");
        command
            .arg(&script_path)
            .arg(pid.to_string())
            .current_dir("/opt/selectdb");

        let output = executor::execute_command(&mut command, self.name())?;

        // Write output to file
        fs::write(&output_path, &output.stdout).map_err(CliError::IoError)?;

        Ok(ExecutionResult {
            output_path,
            message: "Process stack trace completed successfully".to_string(),
        })
    }
}

impl PstackTool {
    /// Ensures the pstack script exists at the specified path
    fn ensure_pstack_script(&self, script_path: &PathBuf) -> Result<()> {
        if script_path.exists() {
            return Ok(());
        }

        // Create the directory if it doesn't exist
        if let Some(parent) = script_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Create the pstack script content
        let script_content = r#"#!/bin/bash
if (( $# < 1 ))
then
    echo "usage: `basename $0` pid" 1>&2
    exit 1
fi

if [[ ! -r /proc/$1 ]]
then
    echo "Process $1 not found." 1>&2
    exit 1
fi

backtrace="bt"
if [[ -d /proc/$1/task ]]
then
    if [[ `ls /proc/$1/task 2>/dev/null | wc -l` > 1 ]]
    then
        backtrace="thread apply all bt"
    fi  ;
elif [[ -f /proc/$1/maps ]]
then
    if grep -e libpthread /proc/$1/maps > /dev/null 2>&1
    then
        backtrace="thread apply all bt"
    fi
fi

GDB=gdb

$GDB -quiet -nx /proc/$1/exe -p $1 <<<"$backtrace" |
    sed -n  \
    -e 's/^(gdb) //' \
    -e '/^#/p' \
    -e '/^Thread/p'
"#;

        fs::write(script_path, script_content)?;

        // Make the script executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(script_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(script_path, perms)?;
        }

        Ok(())
    }
}
