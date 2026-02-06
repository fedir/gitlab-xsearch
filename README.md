# gitlab-xsearch

`gitlab-xsearch` is a high-performance Rust CLI tool for "Transversal Search" across GitLab projects. It allows you to search for code strings across all projects you have access to (membership or group-level) without needing to clone them locally.

## Features

- **Concurrent Search**: Processes multiple project search requests in parallel for maximum speed.
- **Smart URL Handling**: Automatically normalizes GitLab URLs (appends `/api/v4` if missing).
- **Environment Support**: Loads configuration from `.env` or environment variables.
- **Robust Rate Limiting**: Automatic retries with exponential backoff for `429 Too Many Requests`.
- **Progress Visibility**: Real-time progress bar and explicit retry notifications.
- **Flexible Output**: Supports Table (terminal), Markdown, CSV, and Excel (`.xlsx`) formats.
- **Snippet Context**: Includes matching code snippets in the output.

## Installation

### From Source
```bash
cargo build --release
cp target/release/gitlab-xsearch /usr/local/bin/
```

## Configuration

The tool requires a GitLab Personal Access Token (PAT) with `read_api` (and optionally `read_repository`) scope.

You can provide configuration via command-line arguments or environment variables (`.env` file supported):

```env
GITLAB_TOKEN=your_token_here
GITLAB_URL=https://gitlab.example.com
```

## Usage

### General Options
```bash
gitlab-xsearch --query "search_string" [GLOBAL_OPTIONS] <COMMAND> [COMMAND_OPTIONS]

Global Options:
  -q, --query <QUERY>    Search string
      --token <TOKEN>    Personal Access Token (or GITLAB_TOKEN env)
      --url <URL>        GitLab API URL (or GITLAB_URL env)
      --format <FORMAT>  Output format: table, markdown, csv, excel [default: table]
  -o, --output <FILE>    Output file path
```

### Examples

#### 1. Search across all projects (Global)
Search for "password" across all projects you are a member of:
```bash
gitlab-xsearch --query "password" global
```

#### 2. Limit Global search to first N projects
Useful for testing or quick verification:
```bash
gitlab-xsearch --query "func" global --max 20
```

#### 3. Search within a specific Group
Search for "TODO" within group ID `1234` (including subgroups):
```bash
gitlab-xsearch --query "TODO" group 1234
```
You can also limit the number of projects in a group search:
```bash
gitlab-xsearch --query "TODO" group 1234 --max 10
```

#### 4. Export to Excel
```bash
gitlab-xsearch --query "deprecated_api" --format excel -o results.xlsx global
```

#### 5. Generate a Markdown report
```bash
gitlab-xsearch --query "copyright" --format markdown -o report.md global
```

#### 6. Combined Example (Excel + Max Limit + URL)
```bash
gitlab-xsearch --token 'mytoken' --query 'func' --url 'https://gitlab.company/' --format excel -o found.xls global --max 5
```

## Development

Requires Rust and Cargo.

```bash
# Run tests
cargo test

# Build debug
cargo build

# Build release
cargo build --release
```

## License
MIT
