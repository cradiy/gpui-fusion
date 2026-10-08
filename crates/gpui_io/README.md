# gpui_io

File handles, asynchronous read/write sessions, and platform storage locations.
The crate has no GPUI, window, renderer, or async-runtime dependency. Blocking
operations use an injected `IoExecutor`; platform adapters supply document and
browser resources.

```rust,ignore
let io = FileSystem::desktop("dev.example.editor", executor)?;
let data = io.location(SystemLocation::AppData).await?;
let file = data.create_file("project.json", CreateOptions::default()).await?;
let mut writer = file.open_write(WriteOptions::truncate()).await?;
writer.write_all(&contents).await?;
writer.close().await?;
```

See [file I/O](docs/file_io.md) for sessions, storage locations, and platform limits.
