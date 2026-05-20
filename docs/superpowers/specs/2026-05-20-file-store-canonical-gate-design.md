# File Store Canonical Gate Design

## Purpose

Add a production gate that lets embedders and CLI users require a file-backed store to already be in the current canonical durable format before accepting it for use.

ContinuityDB can now report file-store health, including whether compaction is recommended. The next production step is to make that report enforceable. A deployment should be able to fail early when a readable store is legacy raw, checksum-free, or otherwise compaction-worthy.

## Design Options Considered

1. Add canonical format to `KernelRequirements`.
   - Pros: one requirement mechanism.
   - Cons: canonical format is specific to `FileKernel` and its JSONL migration path. Future kernels will have different maintenance dimensions.

2. Add a file-specific canonical gate in the native API and CLI.
   - Pros: explicit, small, and built on `FileKernelHealth`. It does not pollute the generic storage trait.
   - Cons: callers must use a file-specific API when they care about file format health.

3. Automatically compact during open.
   - Pros: fewer manual steps for callers.
   - Cons: hidden mutation during inspection/open is too risky for a database boundary. Operators should choose when to rewrite durable storage.

Recommended approach: option 2. Keep readable compatibility broad, but add explicit file-specific enforcement for production deployments.

## Native API

Add a typed error to `ContinuityError`:

```rust
FileStoreCompactionRecommended {
    health: FileKernelHealth,
}
```

Add methods on `ContinuityDb<FileKernel>`:

```rust
pub fn ensure_file_store_canonical(&self) -> Result<(), ContinuityError>
pub fn open_canonical_file<P: AsRef<Path>>(path: P) -> Result<Self, ContinuityError>
```

`ensure_file_store_canonical` reads `self.file_store_health()`. If `health.compaction_recommended` is true, it returns the new error with the full health report. Otherwise it returns `Ok(())`.

`open_canonical_file` opens the file using the existing `open_file` helper, then calls `ensure_file_store_canonical` before returning the database handle. It does not compact automatically.

## CLI

Extend `continuitydb inspect-kernel` with:

```text
--require-canonical
```

When set, the command should:

- open the file normally;
- compute capabilities, status, and health as before;
- call the canonical gate;
- return a nonzero exit if compaction is recommended.

The command does not compact the store. The existing `compact-file` command remains the explicit remediation path.

The failure path may print the typed error through the existing CLI error mechanism. The successful JSON output remains unchanged except for continuing to include status and health.

## Testing

Kernel does not need new tests because the gate is API/CLI behavior built on already-tested `FileKernelHealth`.

API tests should cover:

- canonical new stores pass `ensure_file_store_canonical`;
- legacy raw stores fail with `FileStoreCompactionRecommended`;
- `open_canonical_file` rejects a readable legacy raw store.

CLI tests should cover:

- `inspect-kernel --require-canonical` succeeds for a new canonical store;
- `inspect-kernel --require-canonical` fails for a readable legacy raw store and mentions compaction recommendation.

## Non-Goals

- Do not add automatic compaction on open.
- Do not reject legacy readable stores by default.
- Do not add a generic `StorageKernel` health or canonicality requirement.
- Do not add canonical gates to export, import, or compaction commands in this slice.

