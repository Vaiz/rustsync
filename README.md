# Tokio FS Example

This project demonstrates how to use `tokio::fs` for asynchronous file operations in Rust. It includes functionality to compare and copy directories using the Tokio runtime, with support for parallel file operations.

## Features

- **Asynchronous File Operations**: Uses `tokio::fs` for reading and writing files asynchronously.
- **Parallel Processing**: Utilizes Tokio's concurrency model to process multiple files simultaneously.
- **Recursive Directory Handling**: Can recursively process directories if specified.

## Usage

### Command-Line Arguments

- `--recursive, -r`: Recurse into directories.
- `--mkpath`: Create destination's missing path components.
- `--source <SOURCE>`: Source directory.
- `--target <TARGET>`: Target directory.

### Example

```sh
cargo run -- --recursive --mkpath /path/to/source /path/to/target
```