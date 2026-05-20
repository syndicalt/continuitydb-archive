# Conditional File Compaction Design

## Purpose

Add an explicit maintenance path that compacts a file-backed store only when its health report recommends compaction.

ContinuityDB now exposes file-store health and a canonicality gate. The remaining operational gap is remediation ergonomics: operators can detect that a readable store needs compaction, but automation must either always rewrite the file or duplicate the health check. This slice adds a native helper and CLI option for "compact if needed" without making open or inspect mutate storage.

## Design Options Considered

1. Automatically compact during `open_canonical_file`.
   - Pros: one call repairs and opens.
   - Cons: hidden mutation during open is too risky for a datastore boundary.

2. Add `compact_file_store_if_needed` as an explicit maintenance operation.
   - Pros: mutation remains explicit, callers get a deterministic summary, and canonical stores are not rewritten.
   - Cons: another file-specific helper on the native API.

3. Only add a CLI option.
   - Pros: enough for operators.
   - Cons: embedders would still duplicate the same health-check and compaction orchestration.

Recommended approach: option 2 plus CLI exposure. Keep the operation file-specific and explicit.

## Native API

Add `FileCompactionSummary` in `continuitydb-api`:

```rust
pub struct FileCompactionSummary {
    pub compacted: bool,
    pub before: FileKernelHealth,
    pub after: FileKernelHealth,
}
```

Add:

```rust
pub fn compact_file_store_if_needed(&mut self) -> Result<FileCompactionSummary, ContinuityError>
```

Behavior:

- Capture `before = self.file_store_health()`.
- If `before.compaction_recommended` is false, return `compacted: false` and `after: before`.
- If compaction is recommended, call the existing `compact_file_store()`.
- Capture `after = self.file_store_health()`.
- Return `compacted: true`, `before`, and `after`.

The helper does not swallow compaction errors. Existing corruption errors still surface during open before callers can invoke this helper.

## CLI

Extend:

```text
continuitydb compact-file <path> --if-needed
```

When `--if-needed` is omitted, current behavior remains: compact unconditionally and print `compacted: true`.

When `--if-needed` is set, call the native conditional helper and print:

```json
{
  "path": "...",
  "compacted": false,
  "before": { "compaction_recommended": false, ... },
  "after": { "compaction_recommended": false, ... }
}
```

The existing unconditional command can keep its current simple output plus optional health fields are not required in this slice.

## Testing

API tests:

- canonical new stores return `compacted: false` and preserve identical before/after health;
- legacy raw stores return `compacted: true` and after-health no longer recommends compaction.

CLI tests:

- `compact-file --if-needed` on a new store skips compaction and reports `compacted: false`;
- `compact-file --if-needed` on a legacy store compacts and reports `compacted: true` with after-health canonical.

## Non-Goals

- Do not compact during open or inspect.
- Do not add scheduling, background maintenance, or watchers.
- Do not add the conditional helper to the generic `StorageKernel` trait.
- Do not change unconditional `compact-file` behavior.

