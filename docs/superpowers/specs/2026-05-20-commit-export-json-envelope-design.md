# Commit Export JSON Envelope Design

## Goal

Make native commit export batches portable outside process memory by adding a deterministic, versioned JSON envelope.

## Current Behavior

`CommitExportBatch` can be produced and imported in memory. It cannot yet be safely written to a file, sent over a sync channel, or inspected by a CLI with a stable format marker.

## Proposed Behavior

Add a versioned envelope:

```rust
pub struct CommitExportEnvelope {
    pub format: String,
    pub version: u32,
    pub batch: CommitExportBatch,
}
```

Constants:

```rust
COMMIT_EXPORT_FORMAT = "continuitydb.commit_export"
COMMIT_EXPORT_FORMAT_VERSION = 1
```

Add API helpers:

```rust
CommitExportEnvelope::new(batch)
CommitExportEnvelope::validate()
ContinuityDb::encode_commit_export_json(batch)
ContinuityDb::decode_commit_export_json(bytes)
```

Decode must reject unsupported format/version values with a deterministic `ContinuityError::InvalidCommitExportEnvelope`.

## Non-Goals

- Do not add CLI export/import commands in this slice.
- Do not add streaming or compression.
- Do not change the commit export/import semantics.
- Do not make the wire format configurable.

## Tests

Add API tests proving:

- exported batches encode as a JSON envelope with the expected format/version;
- decoding the JSON envelope preserves the batch exactly;
- unsupported envelope versions are rejected;
- decoded batches can be imported into another store.
