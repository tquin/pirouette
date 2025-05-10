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
      - "/your/path/to/config:/config"
```
### Binary from Cargo

Alternatively, if you can't run a container, pirouette is also available as a binary Rust crate.

`cargo install pirouette`

## Configuration

Configuration for pirouette is done through a `pirouette.toml` file. By default, pirouette will look for this in the current working directory, or `/config` for a container. You can override this by specifying a full path in the environment variable `PIROUETTE_CONFIG_FILE`.

### Source & Target

These sections specify the path to the data you want to snapshot, and where they should be stored. If using Docker, you can leave these as `/source` and `/target` and map them to the corresponding host paths in your Compose file.

`source` can point to either a single file or a directory. `target` must be a directory, but if it doesn't already exist, pirouette will create it for you.

### Retention

This section defines how many copies of the source data pirouette should keep at different ages. You can specify any combination of `hours`, `days`, `weeks`, `months`, and `years`, and can exclude any time intervals you don't want to use.

### Options

- `output_format` - either `directory` or `tarball`. Determines whether snapshots of directories retain their original file structure, or are compressed into a single `.tgz` file.

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
```


## Todo
- src: deep copy? shallow? latest file only?
- glob include/exclude patterns would be nice at some point too.
- custom-defined retention periods would be nice
- dry-run option?
- one-shot or background daemon mode?
