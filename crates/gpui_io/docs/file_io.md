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
It does not guarantee durable storage or cloud synchronization. Only an explicit
atomic-replacement session guarantees atomic publication.
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

## Atomic replacement

Use `can_write_atomically()` to query provider support, then request an atomic
session explicitly:

```rust,ignore
let mut writer = file.open_write(WriteOptions::atomic_replace()).await?;
writer.write_all(&contents).await?;
writer.close().await?;
```

Writes and `flush()` leave the destination unchanged. Successful `close()` publishes
the complete contents, including an empty file. `abort()` discards staged data;
dropping an unfinished writer schedules cleanup. If close has already been
dispatched, cancellation cannot prevent publication. Await close before reporting
a successful save. Concurrent saves use last-completed replacement; there is no
conflict detection.

Native path handles on Linux, macOS, Windows and Android support this mode,
including Android application-private files. A temporary file is created in the
destination directory, so that directory must be writable and already exist.
The destination may be absent. Existing symbolic links, directories and read-only
files are rejected. Basic permissions are preserved; file identity, hard-link
relationships, ownership, ACLs and extended attributes are not preserved. Other
open handles may continue reading the previous file.

Atomic visibility is distinct from crash durability. Native writes synchronize
the staged file before replacement, but do not synchronize the parent directory.
A process crash can leave a temporary file. Filesystem and access errors are
reported; support queries do not guarantee that a later write succeeds.

Android document/MediaStore handles and browser file handles do not support atomic
replacement. They return `ErrorKind::Unsupported` without modifying contents.
There is no automatic fallback to a truncating write. `write()` and `write_stream()`
retain their ordinary truncating semantics.

## Reading directories and opening existing files

`read_dir()` lists immediate children on an I/O worker. An empty relative path
selects the location itself; a path such as `"Archive/2026"` selects a child
directory. Results contain each child's `name` and `DirectoryEntryKind` and are
not sorted or recursive. `open_file()` looks up an existing file without creating
it. Both work with native directories and Android document-tree grants.

```rust,ignore
let entries = directory.read_dir("Archive").await?;
for entry in entries {
    if entry.kind == DirectoryEntryKind::File {
        let file = directory.open_file(std::path::Path::new("Archive").join(entry.name)).await?;
        let metadata = file.metadata().await?;
    }
}
```

Paths follow the relative-component restrictions of `create_file()`. Native
names are `OsString` values so non-UTF-8 names can be passed back without lossy
conversion. Android document paths require UTF-8. Native listings identify
symbolic links without following them; opening a file or listing a directory
through a symbolic link follows normal filesystem behavior. A location is not
a filesystem sandbox.

A listing is collected in memory and can become outdated while storage changes.
Missing entries, lost access, and file/directory mismatches return errors.
Android providers that report loading or query errors return an error rather
than an incomplete listing; retry when appropriate. Name lookup rejects ambiguous
duplicate display names. These APIs do not prompt for permission or query
MediaStore collections. Use a selected document tree for directory browsing.

The synchronous `file()` helper only constructs a native path handle and does
not check existence. Use `open_file().await` for lookup across providers.

## Copying, moving, renaming, and deleting files

Use a file handle as the source and a location as the destination. Destination
paths follow the same relative-path rules as `create_file()`; missing parent
directories are created. Existing files are never overwritten, but a provider
may choose a different display name. Use the returned handle as the result.

```rust,ignore
let copy = source.copy_to(&directory, "Archive/report.pdf", CreateOptions::default()).await?;
let moved = copy.move_to(&downloads, "Reports/report.pdf", CreateOptions::default()).await?;
if moved.can_delete().await? {
    moved.delete().await?;
}
```

`copy_to()` streams file contents in chunks of at most 64 KiB. If no MIME type is
provided, it uses the source metadata when available. Native files without MIME
metadata may need an explicit `CreateOptions::mime_type` when copying into a
typed collection such as Pictures. Ownership, permissions, modification times,
and other filesystem metadata are not preserved. These operations handle files,
not recursive directory trees; native moves and deletes reject symbolic links.

`move_to()` queries deletion support before creating a destination, then copies,
closes the reader and writer, and deletes the source. It uses the same byte-copy
path for transfers within and across providers, including native paths, Android
document trees, and application-owned MediaStore files. It is not an atomic
rename and does not use provider-side copy/move acceleration. Do not modify the
source or target concurrently. Completion does not guarantee durable storage
or remote synchronization.
After a move, use the returned handle; other references and bookmarks to the
old location are not updated automatically.

Both operations return `TransferError` on failure:

- `Prepare`: no destination handle was obtained.
- `Copy`: input/output failed. Cleanup attempts to delete the newly created
  output; `destination` is present if cleanup failed and may reference partial data.
- `DeleteSource`: copying completed, but source deletion failed. `destination`
  contains the complete copy. Reconcile the source before retrying the move.

Cancellation releases I/O sessions, but can leave a partial or complete output,
including when creation was already dispatched. No source deletion is started
until copying and closing succeed. Cancellation after deletion was dispatched
cannot undo that operation. Missing parent directories created by a failed
transfer are not removed.

`delete()` permanently removes a file; it does not send it to a trash directory.
Deletion support is independent of content-write access. Android document
providers are checked for [deletion support](https://developer.android.com/reference/android/provider/DocumentsContract.Document#FLAG_SUPPORTS_DELETE);
read-only selections and arbitrary shared content URIs are not deletable through
this API. Access can still change after `can_delete()`. No permission dialog is
opened automatically. Browser-selected files can be copy sources when a writable
destination is supplied by an integration, but do not support deletion or moves.

Use `trash()` to request recoverable removal instead. It never falls back to
`delete()`. Applications choose which operation to offer and whether to ask for
confirmation; `can_trash()` is an advisory capability check, not a reservation.

```rust,ignore
if file.can_trash().await? {
    file.trash().await?;
} else {
    // Let the user or application policy choose what to do.
}
// Permanent removal is a separate, explicit operation:
// file.delete().await?;
```

Linux uses the desktop trash, Windows uses the Recycle Bin, and macOS uses
Finder's Trash through NSFileManager. Native operations accept regular files,
not directories or symbolic links. The volume, permissions or system policy can
still prevent trashing. macOS may require manually moving an item out of Trash
rather than offering Finder's Put Back command.

On Android 11 and later, published MediaStore files created by the application
can be marked as trashed. Document-tree and picker files require Android API 37
and a provider advertising trash support. Private filesystem paths, older
document providers and browser handles return unsupported. The operation does
not open a permission dialog or create an application-owned recycle directory.
Close active readers and writers before trashing. Discard old handles/bookmarks
after success; the provider may change the URI. Retention, expiry and restoration
are controlled by the system/provider; this API does not enumerate or restore
trash. Cancelling an already dispatched operation does not undo it.

`can_rename().await` checks renaming support independently. `rename("report.pdf")`
renames a single file within its current directory and returns its new handle.
The argument is one filename, not a relative path. Renaming uses native OS or
provider operations without copying file contents. Native paths reject existing
targets without overwriting them; Android providers determine conflicts and may
return a different display name. A provider may also change the document URI.
Use the returned handle and persist a new bookmark if needed. Renaming does not
update other handles or previously saved bookmarks. Browser selections do not
support renaming. Native rename support depends on the filesystem, and symbolic
links are rejected.

```rust,ignore
if file.can_rename().await? {
    let renamed = file.rename("report-final.pdf").await?;
    // Retain `renamed` as the application's current file handle.
}
```

## Storage locations

### Chosen directories

Use `App::prompt_for_directory()` to let the user choose a writable destination.
Cancellation returns `None`. Desktop platforms use their directory picker;
Android uses a document-tree grant and requires the GPUiForge `files` feature.
Browsers currently return an unsupported error.

```rust,ignore
let selection = cx.prompt_for_directory();
let files = cx.file_system("com.example.app")?;
// Await in an application task.
if let Some(directory) = selection.await?? {
    let bookmark = directory.persist().await?;
    // Store the serializable LocationBookmark in application settings.
    let restored = files.restore_location(&bookmark).await?;
    let file = restored.create_file("Exports/report.txt", CreateOptions {
        mime_type: Some("text/plain".into()),
    }).await?;
    file.write(b"Hello".to_vec()).await?;
}
```

`persist()` explicitly retains the provider's grant. Dropping handles does not
revoke it. `FileSystem::release_location(&bookmark)` releases persistent access
without deleting files; other handles can share that grant. Restoring a revoked
grant or a missing directory returns an error. Native directory bookmarks retain
a path, not sandbox permissions; serialized native paths must be valid Unicode.

Android directories support nested `create_file()` calls without exposing native
paths. Synchronous `file()` lookup is not supported for document trees. Android
11 and later restrict selection of storage roots, the Download root, and protected
Android directories. Choose a permitted subdirectory; see the
[Android directory access guide](https://developer.android.com/training/data-storage/shared/documents-files#grant-access-directory).

### System locations

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

`LocationHandle::create_file(relative_path, options)` creates a new file without overwriting
an existing one. Native locations create the directory if necessary and reject an
existing filename. Providers may adjust the requested display name; use the returned
handle's `name()`. Creation followed by cancellation may leave an empty
native file. `file(relative_path)` resolves a reference without creating it where name lookup
is supported. `path()` is optional.

Both methods accept a filename or a relative file path, such as
`MyApp/Exports/report.txt`. Use `/` separators on every platform. Absolute paths,
empty components, `.` and `..` are rejected. Components must not contain backslashes,
colons or NUL, or end in dots or spaces. `file()` performs no storage access;
`create_file()` creates missing parent directories. Native locations follow
filesystem symlinks normally; a location handle is not a filesystem sandbox.

```rust,ignore
let downloads = files.location(SystemLocation::Downloads).await?;
let file = downloads.create_file("MyApp/Exports/report.txt", CreateOptions {
    mime_type: Some("text/plain".into()),
}).await?;
file.write(contents).await?;
```

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

Public collection subdirectories map to MediaStore's `RELATIVE_PATH` under the
selected collection's standard directory. For example, creating
`MyApp/Exports/report.txt` in Downloads creates an item under `Download/MyApp/Exports/`.
This does not expose a native path, enumerate existing files, or change access
permissions. Providers may normalize directory or file names.

New collection items remain pending until their first writer closes successfully.
Aborting that writer or dropping all handles and sessions before publication
attempts to remove the pending item. Cleanup failures are reported by `abort()`
or logged during drop; process termination cannot guarantee immediate cleanup.
Published files and ordinary picker documents are not deleted on write failure.
Android document handles allow one active writer at a time. Handle access grants
are persisted only by an explicit `persist()` call.

## File bookmarks

`file.persist().await?` retains supported platform access and returns a serializable
`FileBookmark`. Store it in application settings or a database using Serde. A
bookmark identifies its provider and contains opaque bytes; do not edit its payload.
It does not contain the document contents or independently grant access to a file.

`file_system.restore_file(&bookmark).await?` returns a file handle after validating
the platform grant and file availability. `release_file(&bookmark).await?` releases
retained access without deleting the file. Dropping a bookmark or file handle does
not release permissions. The application owns bookmark persistence and cleanup.

Android document-picker handles support bookmarks when the provider offers
persistable grants. Read-only handles retain only read access. Grants are shared
within an application, so release can affect other bookmarks for the same document;
temporary grants and open streams may remain usable. Restore never silently reduces
the requested access. Deleted documents, revoked grants and unavailable providers
return errors. See [Android hosting](../../gpui_android/docs/hosting.md#persistent-file-access).

Native path handles, Android public collection handles and browser files currently
return `Unsupported` from `persist()`. Save ordinary native paths separately where
the operating system does not require a persistent document grant. Bookmarks are
platform-specific and are not portable between apps or devices.

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
