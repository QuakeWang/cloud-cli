# cloud-cli

`cloud-cli` is a command-line tool designed to simplify the management and diagnosis of server-side applications. It provides an interactive menu to access various diagnostic tools, making it easier for developers and system administrators to troubleshoot processes.

## Features

The tool is organized into two main categories:

### FE (Frontend/Java Applications)

- **`jstack`**: Prints Java thread stack traces for a given Java process, helping to diagnose hangs and deadlocks.
- **`jmap`**: Generates heap dumps and provides memory statistics for a Java process, useful for analyzing memory leaks.

### BE (Backend/General Processes)

- **`pstack`**: Displays the stack trace for any running process, offering insights into its execution state.
- **`get_be_vars`**: Retrieves and displays the environment variables of a running process.

## Usage

To run the application, execute the binary. An interactive menu will appear, allowing you to select the desired diagnostic tool.

```sh
./cloud-cli
```

## Releases

This project uses GitHub Actions to automatically build and release binaries for Linux (`x86_64` and `aarch64`). When a new version is tagged (e.g., `v1.0.0`), a new release is created.

You can download the latest pre-compiled binaries from the [GitHub Releases](https://github.com/QuakeWang/cloud-cli/releases) page.
