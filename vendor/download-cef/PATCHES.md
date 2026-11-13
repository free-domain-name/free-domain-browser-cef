# Patches

`src/lib.rs` only: `CefIndex::download_from` raises ureq's 10 MB body limit to
64 MB. CEF's `index.json` is currently about 10.6 MB, so an unpatched clean
build fails with `response body is larger than request limit: 10485760`.

Everything else is upstream `download-cef` 3.0.0 (MIT OR Apache-2.0).
