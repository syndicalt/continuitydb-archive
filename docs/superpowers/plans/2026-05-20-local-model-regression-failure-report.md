# Local Model Regression Failure Report Plan

- [x] Add a failing CLI test for `--fail-on-regression --failure-report-path` without `--artifact-dir`.
- [x] Update the regression gate branch to build the report before returning an error.
- [x] Write the report to `--failure-report-path` when requested.
- [x] Preserve existing artifact bundle behavior.
- [x] Update README and roadmap current scope.
- [ ] Run focused and full verification.
