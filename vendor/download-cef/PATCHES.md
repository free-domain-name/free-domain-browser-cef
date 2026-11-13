# Patches

`src/lib.rs` only:

* `CefIndex::download_from` raises ureq's 10 MB body limit to 64 MB. CEF's
  `index.json` is currently about 10.6 MB, so an unpatched clean build fails
  with `response body is larger than request limit: 10485760`.
* Archive downloads use system `curl` first, with restart/resume retries, and
  fall back to ureq. The CDN occasionally closes large ureq connections with
  `Peer disconnected`; curl resumes instead of starting over.

Everything else is upstream `download-cef` 3.0.0 (MIT OR Apache-2.0).
