# GPUiForge

An independent command-line tool for generating, building and running GPUI
applications. Projects use `gpuiforge.toml`; Android templates and build support
are bundled in the executable.

Install from the repository root:

```sh
cargo install --path tools/gpuiforge
```

Android projects can be generated automatically or exported for manual
maintenance. Desktop and Web commands are configured as process steps.
See [configuration and workflows](docs/usage.md) for setup and project ownership.
