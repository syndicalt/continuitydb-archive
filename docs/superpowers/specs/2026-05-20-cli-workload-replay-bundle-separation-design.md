# CLI Workload Replay Bundle Separation Design

## Problem

`replay-workload --replay-artifact-dir` is intended to archive replay evidence separately from the original workload artifact bundle. If the replay output directory is the same as `--artifact-dir`, replay writes new files into the source fixture directory and weakens the immutability boundary for reproducible storage-engine trials.

## Design

- Reject `replay-workload` invocations where `--replay-artifact-dir` equals `--artifact-dir`.
- Return a clear error before replay reads or writes artifacts.
- Keep `--report-path` and `--failure-report-path` behavior unchanged because explicit single-file outputs may intentionally live wherever operators choose.

## Test

- Add a CLI test that creates a workload bundle and then runs `replay-workload --artifact-dir <dir> --replay-artifact-dir <dir>`, asserting a non-zero exit and no replay manifest creation in the input bundle.
