# Local Model Instability Failure Report Plan

- [x] Add a failing CLI test for `--fail-on-unstable --failure-report-path` without `--artifact-dir`.
- [x] Update the instability gate branch to build a report before returning an error when a report path is requested.
- [x] Write the report to `--failure-report-path` while preserving bundle behavior.
- [x] Update README and roadmap current scope.
- [x] Run focused and full verification.
