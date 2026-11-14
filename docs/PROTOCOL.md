# LocalDrop wire protocol v1

IPv4, TCP 45831, HTTPS only. Windows is the local endpoint and iPhone initiates all connections, including reception of Windows-originated files. No external coordination service, discovery scan or NAT traversal.

QR/manual JSON: `{"v":1,"host":"192.168.1.2","port":45831,"fingerprint":"<64 lowercase hex SHA256 of leaf DER certificate>","token":"<64 hex invitation>"}`. Identity and approved session persist using Windows DPAPI and iOS Keychain; invitation rotates on Windows restart. Trust comes from reading the QR on the physical computer. Never ignore TLS validation globally. The one pinned leaf replaces public CA/hostname trust for this one endpoint. Redirects are rejected.

| Request | Meaning |
|---|---|
| POST /v1/pair | JSON token/name. Waits up to 60s for Windows approval. Returns JSON session/name. One pending request and one active peer. |
| GET /v1/next | Bearer session. Returns oldest envelope or 204 after up to 50s waiting on Notify. No periodic LAN scanning. |
| GET /v1/file/{id} | Bearer session. Streams snapshot, Content-Length. UUIDs are server generated. |
| POST /v1/ack/{id} | Acknowledges head only after verified durable local receipt; removes snapshot. Idempotent. |
| POST /v1/upload?name=...&size=...&sha256=... | Bearer session. Raw file body. Name query is UTF-8 percent encoded. SHA256 and exact byte length required. 200 JSON saved name after verified commit. |
| POST /v1/text | Bearer session. JSON text, max 1 MiB UTF-8. 204 after GUI delivery is queued. |

Envelope: id/name/size/sha256/text/index/count. `text:null` denotes a file; a text envelope has text populated, no file body. Ordered queue capped at 128. Windows outbound SHA256 is calculated while creating immutable staging snapshots. Sessions reconnect across restarts using secrets protected by DPAPI and Keychain; tokens are never logged. Pairing permits are limited; users should reject unexpected requests even if the sender name looks familiar.

64 KiB application IO blocks. Upload receiver rejects excess length, truncated body, wrong hash and 60s without body data. Failed incoming writes remain under UUID `.part` names only in staging; RAII removes them. NTFS hard link publishes verified content without replacing conflicts. A crash can leave staging files, cleaned at next exclusive startup. No destination is exposed under a finished filename until verification succeeds. iOS uses OS download temporary files, verifies with 64 KiB reads, then moves into Documents under a sanitised, non-conflicting name. Storage failure is an error, never success.

An interrupted sender can lose the success response after the receiver has committed: retrying can create another verified copy with a conflict suffix. This protocol provides at-least-once delivery, not exactly-once or byte-offset resume. iOS remembers acknowledged download IDs during its lifetime to avoid duplicates when an ACK response is lost. Pairing is persisted securely, but ACK IDs are not persisted across termination.

GET /v1/hello returns only {"v":1} over pinned HTTPS, with no authentication or device information. iOS performs an 8-second preflight before asking for Windows confirmation. Upload index/count query parameters default to 1/1 and describe original files in a batch, including separate Live Photo resources. Automatic retries target only the saved endpoint while the iOS app is active, with backoff up to 60 seconds; no network scanning is added.

Threat limits: trusted local computer and user-visible QR exchange; no protection from local admin/malware, screen capture of QR or a compromised paired peer. No automatic firewall changes or exposure to public interfaces through port forwarding. Listener binds IPv4 interfaces so users must restrict Windows Firewall to private/local subnet. A paired peer can consume storage; explicit disconnect revokes its token and cancels in-flight transfers.
