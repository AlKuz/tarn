---
aliases:
  - Hypertext Transfer Protocol
tags:
  - programming/web
  - protocols
created: 2024-01-08
---
# HTTP

Hypertext Transfer Protocol - the foundation of web communication.

## Methods

| Method | Description | Idempotent |
|--------|-------------|------------|
| GET    | Retrieve    | Yes        |
| POST   | Create      | No         |
| PUT    | Replace     | Yes        |
| DELETE | Remove      | Yes        |

## Status Codes

- `2xx` - Success
- `3xx` - Redirection
- `4xx` - Client error
- `5xx` - Server error

> [!info] Common Codes
> - 200 OK
> - 201 Created
> - 400 Bad Request
> - 404 Not Found
> - 500 Internal Server Error

## Headers

Important headers for [[wiki/REST API|REST APIs]]:

```http
Content-Type: application/json
Authorization: Bearer <token>
Cache-Control: no-cache
```

## See Also

- [[wiki/REST API]]
- [[wiki/Rust]] - for building HTTP servers
- <https://developer.mozilla.org/en-US/docs/Web/HTTP>

#web #backend
