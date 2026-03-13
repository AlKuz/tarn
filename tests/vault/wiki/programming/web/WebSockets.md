---
tags:
  - programming/web
  - protocols
created: 2024-01-09
---
# WebSockets

Full-duplex communication over a single TCP connection.

## Handshake

Upgrades from [[wiki/HTTP]] connection:

```http
GET /chat HTTP/1.1
Upgrade: websocket
Connection: Upgrade
```

## Use Cases

- Real-time chat
- Live updates
- Gaming

## Implementation

Works well with [[wiki/programming/rust/Async Rust]] for scalable servers.

#realtime #networking
