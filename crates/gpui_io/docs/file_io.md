# File I/O

## Handles and sessions

`FileHandle` identifies a file and its access rights. It exposes `name()`, optional
`path()` and browser `url()`, `can_write()`, and asynchronous `metadata()`.
Metadata fields are optional when a provider cannot supply them. Access can fail
after a handle is obtained, including after permission revocation.
Provider metadata can lag behind completed writes; it is not a read-after-write
content verification mechanism.

`open_read()` returns an independent `FileReader`. `read_chunk()` reads up to
64 KiB; `read_chunk_with_limit(n)` sets another nonzero limit. `None` means EOF.
`open_write(WriteOptions::truncate())` replaces contents, including for an empty
write. `WriteOptions::append()` is supported by native path handles; document
providers may reject it. A native write can create a file, but requires an existing
parent directory. Readers and writers are sequential sessions, not shared cursors.
Await a write's completion before starting another save to the same file.

`FileReader::seek(std::io::SeekFrom)` moves that reader's byte cursor relative to
the start, current position, or end and returns the resulting offset. Native files
and browser files support seeking. Android uses the document's file descriptor;
providers backed by pipes can reject seeking while remaining readable sequentially.
Seeking past EOF is allowed where supported; subsequent reads return `None` until
the cursor is moved back or the file grows. Negative positions return an error.
Seeking does not change any other reader's position or the file's contents.

`reader.read_exact(&mut buffer)` fills the supplied buffer from the current cursor,
continuing across short reads. It returns `UnexpectedEof` if the file ends early;
the buffer can be partially filled and the cursor remains advanced. An empty buffer
performs no I/O.

`reader.read_to_end_limited(max_bytes)` collects all remaining bytes from the current
cursor. It returns `FileTooLarge` if they exceed the limit, consuming at most one
byte beyond the limit to detect overflow. It does not return a truncated result or
rewind on error. A zero limit accepts only a cursor already at EOF. Both helpers
work with sequential readers as well as after `seek`.

`write_all()` writes the whole supplied slice. Native and Android adapters submit
bounded chunks to background workers. `flush()` flushes bytes without finishing
the session. `close()` consumes the writer and reports provider completion errors.
It does not guarantee durable storage, cloud synchronization, or atomic replacement.
`abort()` consumes an unfinished writer and awaits cleanup. Dropping a writer
schedules best-effort cleanup without blocking the caller. Already dispatched
blocking operations cannot be interrupted, and cancelling an operation does not
undo bytes already read or written. Discard a session after cancelling one of its
in-progress operations.

`read()` collects the entire file. `write(Vec<u8>)` and `write_stream(stream)` are
convenience methods for a truncating write followed by close. A stream yields
`anyhow::Result<Vec<u8>>`; it is polled on the caller's task, with the next chunk
requested only after the previous write completes. Input errors stop polling and
abort the output. Choose bounded chunk sizes in the producer. Errors and cancellation
can leave existing files partially written; no automatic rollback is promised.

`file.read_limited(max_bytes)` opens a separate reader at the beginning and collects
the complete file only if it fits within the given byte limit. It uses
`read_to_end_limited`, reading at most one additional byte to detect overflow, and
returns a `std::io::ErrorKind::FileTooLarge` error rather than truncated contents.
It does not rely on metadata or allocate the entire limit in advance. A zero limit
accepts only empty files. The limit applies to file bytes, not later decoding or
decompression. Use reader sessions for larger files that must be processed in chunks.

```rust,ignore
let text = String::from_utf8(file.read_limited(4 * 1024 * 1024).await?)?;
let mut reader = file.open_read().await?;
reader.seek(std::io::SeekFrom::Start(1024)).await?;
let mut header = [0; 16];
reader.read_exact(&mut header).await?;
let remaining = reader.read_to_end_limited(4096).await?;
```

## Storage locations

`FileSystem::desktop(app_id, executor)` discovers desktop locations through `dirs`.
`app_id` must be a stable ASCII application identifier, not a display name or path.
GPUI applications can obtain the platform adapter with `App::file_system(app_id)`.
Location lookup never displays a permission prompt or silently chooses another location.

| Location | Linux default | Windows desktop | macOS default |
| --- | --- | --- | --- |
| AppData | `~/.local/share/<id>` | LocalAppData / `<id>/Data` | `~/Library/Application Support/<id>/Data` |
| AppConfig | `~/.config/<id>` | LocalAppData / `<id>/Config` | `~/Library/Application Support/<id>/Config` |
| Cache | `~/.cache/<id>` | LocalAppData / `<id>/Cache` | `~/Library/Caches/<id>` |

Linux honors the XDG directory settings. Public Downloads, Documents, Pictures,
Music, and Videos use the user's configured locations without appending the app ID.
Missing locations return errors. Desktop app directories do not imply sandbox
isolation, automatic uninstall cleanup, or exclusion from backup.

`LocationHandle::create_file(name, options)` creates a new file without overwriting
an existing one. Native locations create the directory if necessary and reject an
existing filename. Providers may adjust the requested display name; use the returned
handle's `name()`. Names must be single path components, without separators, colons,
NUL, or trailing dots or spaces. Creation followed by cancellation may leave an empty
native file. `file(name)` resolves a reference without creating it where name lookup
is supported. `path()` is optional.

## Android

AppData maps to `filesDir/Data`, AppConfig to `filesDir/Config`, and Cache to
`cacheDir`. The installed application's package determines the sandbox; the
desktop app ID does not create another sandbox. These locations need no storage
permission. `AndroidPlatform::no_backup_directory()` exposes `noBackupFilesDir`.
App-specific files are removed on uninstall; cache files can be removed earlier.

On Android 10 and later, Downloads, Pictures, Music, and Videos expose MediaStore
collections. Creating files requires a concrete `CreateOptions::mime_type`;
Pictures requires an image MIME type, Music audio, and Videos video.
These locations have no native path or name lookup. Creating a new item does not
grant access to existing items owned by other apps. Documents requires explicit
selection through the system document picker. Older Android versions report
unsupported public collections; they do not request broad storage permissions.

New collection items remain pending until their first writer closes successfully.
Aborting that writer or dropping all handles and sessions before publication
attempts to remove the pending item. Cleanup failures are reported by `abort()`
or logged during drop; process termination cannot guarantee immediate cleanup.
Published files and ordinary picker documents are not deleted on write failure.
Android document handles allow one active writer at a time. Handle access grants
are not persisted for use across application restarts.

## Browser

Picked browser files support metadata and incremental reads through Blob slices.
Readers and handles retain the browser resource and its object URL. Operations
must run on its owning browser thread. Writable handles and system locations are
unsupported by the current browser adapter and return errors.

## Platform integration

`IoExecutor` accepts a callback that dispatches blocking work to an application's
worker pool. It must not execute that work inline on the UI thread. Standalone
applications need no GPUI context. `PlatformFile`, `PlatformLocation`, and
`PlatformLocations` implement resources and discovery. `PlatformReader` and
`PlatformWriter` implement async sessions; `from_blocking` adapts blocking streams.
`FileReader::from_seekable` adapts blocking `Read + Seek` sources. A reader backend
without `seek` support returns `Unsupported` through the default implementation.
`BlockingWrite::close` finishes output and `abort` releases unfinished output.
The dispatcher must remain able to execute cleanup while handles or sessions exist.
