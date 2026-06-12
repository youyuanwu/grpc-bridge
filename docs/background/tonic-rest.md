# tonic-rest background

`tonic-rest` ([crates.io](https://crates.io/crates/tonic-rest),
[GitHub](https://github.com/zs-dima/tonic-rest)) lets a single `.proto`
definition serve both gRPC and a REST/JSON API, by code-generating Axum route
handlers that delegate to the same Tonic service traits. This document
captures how the pieces fit together and how we use it in
[tonic-rest-test](../../tonic-rest-test/Cargo.toml).

## Goal

Keep the proto file as the single source of truth. Annotate methods with
[`google.api.http`](https://cloud.google.com/endpoints/docs/grpc/transcoding)
bindings and get:

- Tonic gRPC server + client (via `tonic-prost-build`)
- Axum REST router with JSON transcoding (via `tonic-rest-build`)
- Optional OpenAPI 3.1 spec (via `tonic-rest-openapi`, not used here)

```text
                  .proto + google.api.http
                            │
            ┌───────────────┼───────────────┐
            ▼               ▼               ▼
      Tonic gRPC      Axum REST       OpenAPI 3.1
       handlers        handlers          spec
```

## Crate layout

| Crate              | Role                                                          | Where it lives          |
| ------------------ | ------------------------------------------------------------- | ----------------------- |
| `tonic-rest-core`  | Shared `FileDescriptorSet` types                              | internal                |
| `tonic-rest`       | Runtime helpers (`RestError`, `build_tonic_request`, SSE)     | `[dependencies]`        |
| `tonic-rest-build` | Build-time codegen (proto descriptors → Axum handlers)        | `[build-dependencies]`  |
| `tonic-rest-openapi` | OpenAPI 3.1 spec generator                                  | CLI / CI                |

## Build pipeline

`tonic-rest-build` does not replace `prost-build` / `tonic-prost-build` — it
runs alongside them and consumes the same `FileDescriptorSet`.

The three phases inside [`tonic-rest-test/build.rs`](../../tonic-rest-test/build.rs):

1. **Dump descriptor set.** `dump_file_descriptor_set(PROTO_FILES, INCLUDES, out)`
   invokes `protoc` to produce a binary `FileDescriptorSet`. This is what
   carries the `google.api.http` annotations through to the codegen step.
2. **Generate prost/tonic types.** `tonic_prost_build::configure().compile_protos(...)`
   emits the message structs, server traits, and client stubs the way any
   tonic project would. We add `#[derive(serde::Serialize, serde::Deserialize)]
   #[serde(default)]` via `.type_attribute(".", ...)` so the generated
   messages can be used as the body of Axum `Json` / `Query` extractors.
3. **Generate REST routes.** `tonic_rest_build::generate(&descriptor_bytes, &cfg)`
   walks the descriptor set, finds every method with an `google.api.http`
   option, and produces an Axum router file we `include!` into our crate.

By default no `extra_forwarded_headers` / `extension_type` are set, so the
generated handlers stay minimal.

## Generated code shape

For our [`greeter.proto`](../../tonic-rest-test/proto/greeter.proto) the
codegen emits something like:

```rust
pub fn greeter_rest_router<S>(service: Arc<S>) -> Router
where
    S: crate::greeter::greeter_server::Greeter + Send + Sync + 'static,
{
    Router::new()
        .route("/v1/sayhello",         post(rest_greeter_say_hello::<S>))
        .route("/v1/greeting/{name}",  get(rest_greeter_get_greeting::<S>))
        .with_state(service)
}

async fn rest_greeter_say_hello<S>(
    State(service): State<Arc<S>>,
    headers: HeaderMap,
    Json(body): Json<HelloRequest>,
) -> Result<Json<HelloReply>, tonic_rest::RestError>
where S: greeter_server::Greeter + Send + Sync + 'static,
{
    let req = tonic_rest::build_tonic_request::<_, ()>(body, &headers, None);
    let response = service.say_hello(req).await?;
    Ok(Json(response.into_inner()))
}
```

Key points:

- The router is **generic over the Tonic server trait** (`S: Greeter`). The
  same `impl` that backs the gRPC server backs the REST endpoints — no
  duplicate business logic.
- The body type passed to `Json` / `Query` is the proto message itself
  (made (de)serializable in phase 2).
- `build_tonic_request` copies a fixed set of forwarded HTTP headers
  (`authorization`, `user-agent`, `x-forwarded-for`, `x-real-ip`) into
  `tonic::Request::metadata`, so the service impl sees the same metadata
  regardless of transport.
- A `PUBLIC_REST_PATHS: &[&str]` constant lists paths that should bypass auth
  middleware (configured via `.public_methods(...)` on `RestCodegenConfig`).
- `all_rest_routes(...)` is also emitted as a combined router for projects
  with multiple services.

### Handler variants

The codegen picks an Axum handler shape per HTTP verb:

| HTTP verb         | Request extractor         | Success response          |
| ----------------- | ------------------------- | ------------------------- |
| POST / PUT / PATCH| `Json<T>`                 | `Json<Response>`          |
| GET               | `Query<T>` (+ `Path` vars)| `Json<Response>`          |
| DELETE            | `T::default()`            | `StatusCode::NO_CONTENT`  |
| GET + streaming   | `Query<T>`                | `Sse<impl Stream<...>>`   |

Path parameters declared as `{name}` in the binding are extracted via
`axum::extract::Path` and then written into the corresponding proto field
before the request is forwarded to the gRPC trait. Server-streaming RPCs are
automatically exposed as Server-Sent Events with a keep-alive interval
(`sse_keep_alive_secs`, default 15s); per-item errors are emitted as
structured `event: error` frames via `sse_error_event`.

### How the request body is mapped to the proto

The `google.api.http` `body` field controls request decoding. `tonic-rest`
supports only the two endpoints of that spec — `body: "*"` and `body: ""`:

| `body` value | Codegen behaviour                                                                 |
| ------------ | --------------------------------------------------------------------------------- |
| `"*"`        | Extract the whole proto message from the JSON request body with `Json<T>`.        |
| `""` (or absent), non-GET | No body is read; the handler starts from `T::default()`.             |
| `""`, GET    | The proto message is built from the query string with `Query<T>`.                 |
| `"some_field"` | **Not supported** — codegen returns `GenerateError::UnsupportedBodySelector`.   |

Decoding itself is just `serde_json` going through whatever derives are on
the prost-generated struct. There is no protobuf-aware JSON codec — the
crate doesn't implement [proto3 canonical JSON]
(https://protobuf.dev/programming-guides/json/). That's why phase 2 of our
[build.rs](../../tonic-rest-test/build.rs) attaches
`#[derive(serde::Serialize, serde::Deserialize)] #[serde(default)]` to every
message. For projects with well-known types, the recommended pattern is
`tonic_rest_build::configure_prost_serde(...)`, which wires
`#[serde(with = "tonic_rest::serde::opt_timestamp")]` (and friends) onto
`Timestamp` / `Duration` / `FieldMask` / enum fields so they round-trip in
the canonical formats.

Once the message struct is built, path parameters overwrite their target
fields and the response is encoded back to JSON the same way — `Json(reply)`
runs `serde_json::to_vec` on the prost struct.

### Compatibility with grpc-gateway

Same input, mostly different output. `tonic-rest` and
[grpc-gateway](https://github.com/grpc-ecosystem/grpc-gateway) both consume
`google.api.http` annotations, but they're not wire-compatible:

| Aspect                      | tonic-rest 0.1                                  | grpc-gateway                                     |
| --------------------------- | ----------------------------------------------- | ------------------------------------------------ |
| Annotation source           | `google.api.http`                               | `google.api.http`                                |
| Path templates `{var}`, `{var=segments/*}` | Single-segment vars only          | Full RFC 6570-ish path template grammar          |
| `additional_bindings`       | Ignored (only primary binding)                  | Supported                                        |
| `body: "*"`                 | Supported                                       | Supported                                        |
| `body: "field_name"`        | **Rejected at build time**                      | Supported                                        |
| Query parameters            | `serde_urlencoded` via `Query<T>` (flat fields) | Full proto field-path expansion (`?foo.bar=baz`) |
| JSON codec                  | Plain `serde_json` on prost structs             | proto3 canonical JSON (`protojson`)              |
| WKT encoding (Timestamp, …) | Opt-in via `configure_prost_serde`              | Built-in, canonical (RFC 3339 etc.)              |
| Enum encoding               | `i32` unless you wire `define_enum_serde!`      | Canonical `SCREAMING_SNAKE_CASE` strings         |
| Error envelope              | `{ "error": { code, message, status } }` (Google API error model) | `{ "code", "message", "details" }` (Status proto JSON) |
| Streaming                   | Server-streaming → SSE (`text/event-stream`)    | Server-streaming → newline-delimited JSON        |
| Runtime                     | In-process Axum router                          | Out-of-process reverse proxy to a gRPC backend   |

For simple proto3 schemas with primitive fields the JSON bodies tend to
match, but anything touching well-known types, enums, oneof, `int64`/`uint64`
(which proto3 JSON encodes as strings), or server-streaming will diverge.
Treat tonic-rest as "REST that happens to share a proto file with gRPC",
not as a drop-in replacement for a grpc-gateway endpoint.

## Runtime helpers (`tonic-rest` crate)

The generated code only depends on a handful of small runtime helpers:

- `RestError(tonic::Status)` — `IntoResponse` impl that maps gRPC codes to
  HTTP status codes and renders the
  [Google API error model](https://cloud.google.com/apis/design/errors):

  ```json
  { "error": { "code": 404, "message": "...", "status": "NOT_FOUND" } }
  ```

- `build_tonic_request<T, E>(body, headers, ext)` — assembles a
  `tonic::Request<T>` with forwarded metadata and an optional extension
  (e.g. an `AuthInfo` produced by middleware).
- `sse_error_event(&Status)` — formats a `tonic::Status` as a structured SSE
  `event: error` frame using the same JSON envelope.
- `grpc_to_http_status` / `grpc_code_name` — the underlying status-code
  mapping (all 17 gRPC codes).
- `FORWARDED_HEADERS` / `CLOUDFLARE_HEADERS` — the header sets copied into
  gRPC metadata.
- `serde` module — `#[serde(with)]` adapters for prost well-known types
  (`Timestamp`, `Duration`, `FieldMask`) and a `define_enum_serde!` macro for
  proto3 enums (which are `i32` in prost).

There is **no runtime reflection or extra service layer** — the generated
handlers compile to direct calls into the Tonic trait implementation.

## How our test crate uses it

[`tonic-rest-test`](../../tonic-rest-test/Cargo.toml) is the minimal smoke
test for the integration:

- [`proto/greeter.proto`](../../tonic-rest-test/proto/greeter.proto) defines
  a `Greeter` service with two RPCs — one `POST` with a JSON body and one
  `GET` with a path parameter.
- [`proto/google/api/`](../../tonic-rest-test/proto/google/api/annotations.proto)
  vendors `http.proto` + `annotations.proto` so `protoc` resolves the
  `google.api.http` option without needing the `googleapis` repo on disk.
- [`build.rs`](../../tonic-rest-test/build.rs) runs the three-phase pipeline
  above and writes `$OUT_DIR/rest_routes.rs`.
- [`src/lib.rs`](../../tonic-rest-test/src/lib.rs) implements the `Greeter`
  trait once and exercises it from three async tests:
  - `grpc_say_hello` — Tonic server + generated `GreeterClient`.
  - `rest_say_hello_post` — generated Axum router served via `axum::serve`,
    hit with `reqwest` (`POST /v1/sayhello`).
  - `rest_get_greeting` — same router, `GET /v1/greeting/{name}` with a
    path parameter.

The generated handler file is wrapped in a private module gated with
`#[allow(unfulfilled_lint_expectations)]` because `tonic-rest-build` 0.1.5
emits `#[expect(clippy::too_many_arguments, clippy::needless_pass_by_value)]`
attributes that don't always fire — and we run clippy with `-D warnings` in
CI.

## Toolchain notes

- `tonic-rest` 0.1.x targets `tonic` 0.14 / `axum` 0.8 / `prost` 0.14.
- It currently requires `protoc` on the host (installed via
  [`taiki-e/install-action@protoc`](../../.github/workflows/build.yaml) in
  CI). The `tonic-prost-build` invocation also needs it.
- Generated proto messages need to be serde-compatible. We use the simple
  blanket `.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)] #[serde(default)]")`.
  Production projects typically prefer
  `tonic_rest_build::configure_prost_serde(...)` which wires up
  well-known-type adapters automatically.
- Streaming, custom extension types, and Cloudflare-style forwarded headers
  are supported but not exercised here.

## Limitations (per upstream README)

- `HttpRule.additional_bindings` — only the primary binding per method is
  generated.
- `body: "field_name"` partial body selectors — only `"*"` (full body) and
  `""` (no body) are supported.
- `repeated` well-known types — `configure_prost_serde` doesn't wire serde
  adapters for `repeated google.protobuf.Timestamp` and similar.
