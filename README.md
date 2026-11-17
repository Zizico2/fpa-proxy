# fpa-proxy

Rust 1.99.1 tonic native HTTP/2 gRPC proxy for the independent FPA results stack.
Matches prociv-proxy: separate binary/library crates, clap/env/dotenv settings,
reflection, standard health service, gzip and graceful SIGTERM. Dependencies use
full public Git revisions, with no sibling checkout or protoc requirement.

The adapter uses the Progenitor fpa-ws-client and fpa-portal-client pinned to
fpa-openapi, validates input IDs, maps season slugs instead of WordPress term IDs,
preserves missing versus zero scores, excludes misleading calendar state code 50,
and returns NotFound for empty match envelopes. Dates remain local strings.
Concurrency bounds active RPCs; phase RPCs make two parallel HTTP requests.
HTTP JSON is buffered by generated clients; protobuf responses are size-limited.


```sh
rtk cargo run --locked
rtk just verify
```

Default port: 50201. See `.env.example` and `--help`.
The complete flow is fpa-app → fpa-gateway → fpa-proxy → FPA. Packaging,
Compose, Cloud Run and Vercel releases belong to fpa-infra. No background archive,
push scoring stream or invented match clock is provided. The app polls snapshots;
in-play freshness depends on FPA's publication behavior.
