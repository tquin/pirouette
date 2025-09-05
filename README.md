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

All configuration for pirouette is done through a `pirouette.toml` file. Pirouette will look for this file in this order:

- Value from `PIROUETTE_CONFIG_FILE` environment variable, if set
- If running in a container: `/config/pirouette.toml`
- Otherwise: `${CWD}/pirouette.toml`

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

This section defines how many copies of the source data pirouette should keep at different age intervals. While each individual key is optional and can be excluded, at least one of the keys must be provided.

| Key      | Required | Value                                   |
| -------- | -------- | --------------------------------------- |
| `hours`  | No\*     | An integer number of snapshots to keep. |
| `days`   | No\*     | An integer number of snapshots to keep. |
| `weeks`  | No\*     | An integer number of snapshots to keep. |
| `months` | No\*     | An integer number of snapshots to keep. |
| `years`  | No\*     | An integer number of snapshots to keep. |

\*_At least one key must be provided_

### Options

All options listed below are optional, and if excluded will have a default value.

| Key             | Value                                              | Default     | Notes                                                                                              |
| --------------- | -------------------------------------------------- | ----------- | -------------------------------------------------------------------------------------------------- |
| `output_format` | `directory`<br>`tarball`                           | `directory` | Determines whether snapshots retain their structure, or are compressed into a single `.tgz` file.  |
| `log_level`     | `error`<br>`warn`<br>`info`<br>`debug`<br>`trace`  | `warn`      | Set the logging level.                                                                             |
| `dry_run`       | `true`<br>`false`                                  | `false`     | Determines if file system changes can occur. If `true`, will generate `DEBUG`-level logs instead.  |
| `include`       | List of glob patterns, eg: `["foo.txt", "foo/**"]` | `[]` (None) | Only files in the `source` which match at least one of the `include` patterns will be snapshotted. |
| `exclude`       | List of glob patterns, eg: `["foo/**/badfile"]`    | `[]` (None) | Only files in the `source` which match none of the `exclude` patterns will be snapshotted.         |

## Local Development

You can test changes in a Docker container:

```
./docker-dev.sh
```

## Todo

- custom-defined retention periods would be nice
- one-shot or background daemon mode?
