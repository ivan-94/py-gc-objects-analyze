# Security Policy

## Supported Versions

`py-gc-objects-analyze` is pre-1.0. Security fixes target the latest released 0.x version unless a release note says otherwise.

## Reporting A Vulnerability

Please do not open public issues for vulnerabilities or sensitive dump exposure. Use GitHub private vulnerability reporting for this repository when available:

<https://github.com/ivan-94/py-gc-objects-analyze/security/advisories/new>

If private reporting is unavailable, contact the maintainers through a private channel and include enough detail to reproduce the issue without attaching private dumps.

## Runtime Safety Notes

- `pygco-dump` can expose object metadata from the target Python process.
- Do not expose dump endpoints to untrusted users or the public internet.
- Keep dump routes behind your own internal access controls and operational runbooks.
- `collect=true` triggers GC in the target process and may affect latency.
- `include_repr=true` can run user-defined Python code and may emit sensitive or huge strings.
- `pygco` Web/API servers are local single-user tools and should bind to loopback unless you have a separate trust boundary.

Read [docs/runtime-safety.md](docs/runtime-safety.md) and [docs/producer-integration.md](docs/producer-integration.md) before using the producer in shared or production environments.
