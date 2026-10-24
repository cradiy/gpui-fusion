#[path = "support/graphics_preview.rs"]
mod preview;

use gpui_richtext::{Document, HeadingLevel, Mermaid, MermaidCodeBlock, compose::*};

fn main() {
    preview::run(
        Document::from_roots([
            heading(HeadingLevel::H1).child(text("Diagrams")),
            node(Mermaid(
                r"flowchart LR
    Queue --> Worker
    Worker --> Archive
    Worker --> Retry
    Retry --> Queue
"
                .into(),
            )),
            node(MermaidCodeBlock(
                r"sequenceDiagram
    participant Client
    participant Cache
    Client->>Cache: Fetch record
    Cache-->>Client: Cached result
"
                .into(),
            )),
        ])
        .unwrap(),
    );
}
