#[path = "support/graphics_preview.rs"]
mod preview;

fn main() {
    preview::run(
        r#"# Diagrams

```mermaid
flowchart LR
    Queue --> Worker
    Worker --> Archive
    Worker --> Retry
    Retry --> Queue
```

```mermaid
sequenceDiagram
    participant Client
    participant Cache
    Client->>Cache: Fetch record
    Cache-->>Client: Cached result
```
"#,
    );
}
