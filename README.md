# cloud-cli

`cloud-cli` is an interactive command-line tool for on-call troubleshooting of Apache Doris / SelectDB clusters. It provides a TUI menu that groups common FE/BE diagnostic workflows and writes collected artifacts into an output directory for easy archiving and sharing.

## Features

The tool is organized into two categories: `FE` and `BE`.

### FE

- `fe-list`: List and select the FE target host (IP) for the current session based on `clusters.toml`.
- `jmap` (`jmap-dump`, `jmap-histo`): Java heap dump / histogram.
- `jstack`: Java thread dump.
- `fe-profiler`: Generate a flame graph using Doris `bin/profile_fe.sh` (requires async-profiler).
- `table-info`: Interactive database/table browser that collects and summarizes schema, indexes, partitions, and bucket details (supports exporting `.txt` reports).
- `routine-load`: Routine Load helper tools:
  - `Get Job ID`: List and select a Routine Load job (cached locally for later analysis).
  - `Performance Analysis`: Analyze per-commit rows/bytes/time from FE logs.
  - `Traffic Monitor`: Aggregate per-minute `loadedRows` from FE logs.
- `fe-audit-topsql`: Parse `fe.audit.log`, normalize SQL into templates, and aggregate by CPU/time to generate a TopSQL report. Default filter is `count > 10`; if nothing matches, it falls back to showing results without the count filter.

### BE

- `be-list`: List and select the BE target host (IP) for the current session based on `clusters.toml` (defaults to `127.0.0.1`).
- `pstack`: Use `gdb` to dump process thread stacks (writes a `.txt` file).
- `jmap` (`jmap-dump`, `jmap-histo`): Java heap dump / histogram (only meaningful for JVM processes).
- `be-config`:
  - `get-vars` (`get-be-vars`): Query BE configuration variables via HTTP `/varz`.
  - `update-config` (`set-be-config`): Update configuration via HTTP `/api/update_config` (supports persist).
- `pipeline-tasks`: Fetch running pipeline tasks via HTTP `/api/running_pipeline_tasks` (auto-saves when the response is large).
- `memz`:
  - `current` (`memz`): Fetch the current Jemalloc memory view via HTTP `/memz` (saves HTML and prints a summary).
  - `global` (`memz-global`): Fetch the global memory view via HTTP `/memz?type=global`.

## Usage

### Build and run

```sh
cargo build --release
./target/release/cloud-cli
```

### Configuration

- Persistent config: `~/.config/cloud-cli/config.toml`
- MySQL key file: `~/.config/cloud-cli/key`
- Cluster topology cache: `~/.config/cloud-cli/clusters.toml`
- First run: if an FE process is detected and MySQL credentials are missing, it will prompt you to configure and test the connection. On success, it writes `config.toml` and fetches cluster topology into `clusters.toml` (used by `fe-list`/`be-list`).
- Common environment variables (override persistent config):
  - `JDK_PATH`: Set the JDK path (used by `jmap`/`jstack`).
  - `OUTPUT_DIR`: Set the output directory (default: `/tmp/doris/collection`).
  - `CLOUD_CLI_TIMEOUT`: External command timeout in seconds (default: `60`).
  - `CLOUD_CLI_NO_PROGRESS`: Disable progress animation (`1`/`true`).
  - `MYSQL_HOST` / `MYSQL_PORT`: Set Doris MySQL endpoint (defaults are derived from config; otherwise `127.0.0.1:9030`).
  - `PROFILE_SECONDS`: Override `fe-profiler` collection duration in seconds.
  - `CLOUD_CLI_FE_AUDIT_TOPSQL_INPUT`: Provide an explicit `fe.audit.log` path for `fe-audit-topsql` (skips interactive selection).

### Output

- Most tools write artifacts into the output directory (default: `/tmp/doris/collection`) and print `Output saved to: ...`.
- A few tools primarily print to stdout (e.g., `get-be-vars`), but still follow consistent prompts and error messages.

### Runtime dependencies

The CLI invokes some system commands at runtime. Ensure these are installed and available in `PATH`:

- `mysql` (for cluster info, Routine Load, Table Info, etc.)
- `curl` (for BE HTTP tools)
- `gdb` (for `pstack`)
- `bash`

## Releases

This project uses GitHub Actions to build and publish Linux (`x86_64` / `aarch64`) binaries. When you create a tag (e.g., `v1.0.0`), a corresponding GitHub Release is produced.

You can download the latest prebuilt binaries from GitHub Releases: https://github.com/QuakeWang/cloud-cli/releases
