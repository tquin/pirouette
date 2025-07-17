# tquin/pirouette

A log and backup rotation tool.

⚠️ _alpha software under active development - may break your stuff!_ ⚠️

## Installation

### Docker Compose

The recommended installation method is with Docker Compose. You need to edit the volume mappings for the `source` (what you want to take snapshots of), `target` (where should pirouette store those snapshots), and `config` (where the `pirouette.toml` file can be found). Then, run `docker compose up -d` to start pirouette.

```
services:
  pirouette:
    image: tquin/pirouette:latest
    container_name: pirouette
    environment:
      PIROUETTE_CONFIG_FILE: /config/pirouette.toml
    volumes:
      - "/your/path/to/source:/source"
      - "/your/path/to/target:/target"
      - "/your/path/to/pirouette.toml:/config/pirouette.toml"
```

### Binary from Cargo

Alternatively, if you can't run a container, pirouette is also available as a binary Rust crate.

`cargo install pirouette`

## Configuration

Configuration for pirouette is done through a `pirouette.toml` file. By default, pirouette will look for this in the current working directory, or `/config` for a container. You can override this by specifying a full path in the environment variable `PIROUETTE_CONFIG_FILE`.

### Source

Specifies the source data you want to take snapshots of. If using Docker, you can leave this as `/source` and map it to the corresponding host path in your Compose file.

The path must already exist, or pirouette will return an error.

| Key    | Required | Value                                    |
| ------ | -------- | ---------------------------------------- |
| `path` | Yes      | A path to an existing file or directory. |

### Target

Specifies the destination where you want your snapshots stored. If using Docker, you can leave this as `/target` and map it to the corresponding host path in your Compose file.

If the `target.path` doesn't already exist, pirouette will try to create it for you.

| Key    | Required | Value                  |
| ------ | -------- | ---------------------- |
| `path` | Yes      | A path to a directory. |

### Retention

This section defines how many copies of the source data pirouette should keep at different age intervals. While each key is optional and can be excluded, at least one key in total must be provided.

| Key      | Required | Value                                   |
| -------- | -------- | --------------------------------------- |
| `hours`  | No       | An integer number of snapshots to keep. |
| `days`   | No       | An integer number of snapshots to keep. |
| `weeks`  | No       | An integer number of snapshots to keep. |
| `months` | No       | An integer number of snapshots to keep. |
| `years`  | No       | An integer number of snapshots to keep. |

### Options

All options listed below are optional, and if excluded will have a default value.

| Key             | Value                                             | Default     | Notes                                                                                             |
| --------------- | ------------------------------------------------- | ----------- | ------------------------------------------------------------------------------------------------- |
| `output_format` | `directory`<br>`tarball`                          | `directory` | Determines whether snapshots retain their structure, or are compressed into a single `.tgz` file. |
| `log_level`     | `error`<br>`warn`<br>`info`<br>`debug`<br>`trace` | `warn`      | Set the logging level.                                                                            |
| `dry_run`       | `true`<br>`false`                                 | `false`     | Determines if file system changes can occur. If `true`, will generate `DEBUG`-level logs instead. |

### Example

```
[source]
path = "/source"

[target]
path = "/target"

[retention]
days = 7
weeks = 4
months = 12

[options]
output_format = "tarball"
log_level = "warn"
```

## Local Development

You can test changes in a Docker container:

```
./docker-dev.sh
```

## Todo

- src: deep copy? shallow? latest file only?
- glob include/exclude patterns would be nice at some point too.
- custom-defined retention periods would be nice
- one-shot or background daemon mode?
