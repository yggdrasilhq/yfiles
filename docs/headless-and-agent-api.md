# Headless & Agent Automation API

`yfiles` provides structured CLI commands for autonomous agents to inspect and manipulate directories deterministically.

## 1. JSON CLI Verbs

```bash
# List directory contents with full metadata
yfiles list --path /home/user/workspace --format json

# Filter listing by glob pattern
yfiles list --path /home/user/workspace --glob "*.rs" --format json

# Inspect file attributes
yfiles inspect --path /home/user/workspace/Cargo.toml --format json

# Safe trash operation
yfiles trash --path /home/user/workspace/temp_build.log

# Create directory
yfiles mkdir --path /home/user/workspace/new_folder

# Bulk rename
yfiles rename --path /home/user/photos --pattern "IMG_(\d+)\.jpg" --replace "Vacation_\$1.jpg"
```

## 2. Telemetry and Traceability

All operations emit microsecond-precision events via `ytrace` probes (`yfiles-dir-scan`, `yfiles-file-op`, `yfiles-trash-op`).
