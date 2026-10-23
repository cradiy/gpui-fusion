# Packet inspector

Inspect **structured payloads**, compare *decoded fields*, and follow a [protocol reference](https://www.rfc-editor.org/).

## Processing pipeline

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

![Application icon](IMAGE_SOURCE)

The artwork is loaded from a local file. Text following an image remains selectable.
