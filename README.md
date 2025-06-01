# Documentation Server

A lightweight server that hosts documentation for multiple projects, automatically updating and building documentation on startup. Supports Java (Gradle), Rust (Cargo), and custom build systems.

## Features

- **Multi-project hosting**: Serve documentation for multiple projects from a single server
- **Auto-update**: Fetch latest code from Git repositories on startup
- **Build automation**: Generate documentation using Gradle, Cargo, or [custom commands](#roadmap)
- **URL sanitization**: Automatic path normalization for clean URLs
- **Simple configuration**: Easy setup via TOML configuration file

## Configuration

Create a `config.toml` file in the server's working directory:

```toml
# Base directory for all projects
libs_path = "/path/to/projects"

# Server port (default: 8080)
port = 8080

# Run `git pull` and rebuild documentation on server start (default: false)
update_on_start = true

# Project configurations
[[projects]]
path = "my-java-project"             # Relative path under libs_path
repo = "https://github.com/user/my-java-project.git"
build_system = "gradle"              # Generates docs in build/docs/javadoc

[[projects]]
path = "my-rust-project"
repo = "https://github.com/user/my-rust-project.git"
build_system = "cargo"               # Generates docs in target/doc

[[projects]]
path = "custom-docs-project"
build_system = "custom"
build_command = "make documentation" # Custom build command
```

### Configuration Options

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `libs_path` | Path | **Required** | Base directory for all projects |
| `port` | u16 | 8080 | HTTP server port |
| `update_on_start` | bool | false | Update and build projects on startup |
| `projects` | Array | **Required** | List of project configurations |

#### Project Configuration
| Key | Type | Description |
|-----|------|-------------|
| `path` | String | Project directory relative to `libs_path` |
| `repo` | String | *Optional* Git repository URL for updates |
| `build_system` | String | Build system (`gradle`, `cargo`, or `custom`) |
| `build_command` | String | *Required for custom* Command to build docs |

## Installation

1. **Prerequisites**:
   - Rust toolchain (install via [rustup](https://rustup.rs/))
   - Git
   - Java (for Gradle projects) or Rust (for Cargo projects)

2. **Build from source**:
   ```bash
   git clone https://github.com/your-username/documentation-server.git
   cd documentation-server
   cargo build --release
   ```

## Usage

1. Create a `config.toml` file (see examples above)
2. Run the server:
   ```bash
   ./target/release/documentation-server
   ```
3. Access documentation at:
   ```
   http://localhost:8080
   ```

### Use cases

- Searching documentation without Internet access
- Lower latency for searching documentation
    - You can use a reverse proxy like [Nginx](https://nginx.org/en/) or [Caddy](https://caddyserver.com/) to host it in a dedicated VPS
    - Runs locally so basically 0ms latency, depending on your current CPU usage
- Libraries that aren't on any package registry (like Zig, Java, or C/C++, or Cargo for Rust)

## Endpoints

- `GET /`: Project index page with links to all documentation
- `GET /{project}/`: Documentation for a specific project
- Static files served from generated documentation directories

## How it works

The server is relatively simple, considering what it does. It's derived from my [rudimentary docs server](https://gist.github.com/walker84837/e829c0eef1ec4d8036aa6b1b4a275e14) (which just requires Python and a JVM with optionally Gradle).

1. **Initialization**:
   - Loads configuration from `config.toml`
   - Creates sanitized URL paths for each project
   - Maps documentation output directories

2. **Startup process** (when `update_on_start = true`):
   ```mermaid
   graph TD
     A[Start Server] --> B{update_on_start?}
     B -->|Yes| C[For each project]
     C --> D{Has repo URL?}
     D -->|Yes| E[Update from Git]
     D -->|No| F[Skip update]
     E --> G[Build Documentation]
     F --> G
     G --> H[Start HTTP Server]
     B -->|No| H
   ```

3. **Request handling**:
   - Root path (`/`) shows project index
   - Project paths redirect to documentation index
   - Static files served from build output directories

## Roadmap

Contributions are very welcome!

- Other build systems
    - [ ] Zig ([`zig build-lib -femit-docs src/root.zig`](https://zig.guide/build-system/generating-documentation))  
    - [ ] Kotlin ([Dokka](https://kotlinlang.org/docs/dokka-cli.html))
    - [ ] Scala (`sbt doc`))
    - [ ] C/C++ ([Doxygen](https://www.doxygen.nl))
- Consider moving home page to separate component (which allows for more flexibility and editable from Rust)

## Troubleshooting

**Common issues**:
- **Missing repository**: Projects without `repo` configured will skip update phase
- **Build failures**: Check server logs for build command errors
- **Permission issues**: Ensure server has write access to `libs_path`
- **Missing index.html**: Verify build commands generate documentation in expected locations

**Log levels**: Control verbosity with `RUST_LOG` environment variable:
```bash
RUST_LOG=debug ./documentation-server
```

## License

[MIT License](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE), either at your option.
