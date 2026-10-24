# Packet inspector

Inspect **structured payloads**, compare *decoded fields*, and follow a [protocol reference](https://www.rfc-editor.org/).[^capture]

## Processing pipeline

> [!NOTE]
> Decoded values retain their original byte offsets.

> [!TIP]
> Compare a short capture before opening a large recording.

> [!IMPORTANT]
> Choose the protocol revision before interpreting payload fields.

> [!WARNING]
> An incomplete capture can omit the end of a message.

> [!CAUTION]
> Treat commands extracted from a capture as untrusted data.

### Terminology

Frame
: A complete unit received from the transport.

Payload
: The message content after its framing header.

: Its interpretation depends on the negotiated revision.

### Steps

1. Read the packet header.
   - Check the magic bytes.
   - Validate the payload length.
     - [x] Reject truncated buffers.
     - [ ] Report unknown field identifiers.
2. Decode the payload.

> Keep the original bytes available while inspecting a decoded message.
>
> > A decoder may report partial results.
>
> ```rust
> fn payload(bytes: &[u8]) -> Option<&[u8]> {
>     bytes.get(4..)
> }
> ```

---

## Field summary

| Field | Encoding | Bytes |
| :--- | :---: | ---: |
| Signature | ASCII | 4 |
| Revision | Unsigned integer | 2 |
| Payload | Binary | variable |

## Application artwork

| Preview | Details |
| :---: | :--- |
| [![Application icon](IMAGE_SOURCE "Project artwork")](https://github.com/zed-industries/zed) | **Project artwork** loaded from a local file. Open the preview to visit its source project. |

The artwork is loaded from a local file. Text following an image remains selectable.[^capture]

[^capture]: Capture files retain the transport representation. Decoding produces a separate interpretation without changing those bytes.
