---
aliases:
  - REST
  - RESTful API
tags:
  - programming/web
  - architecture
created: 2024-01-09
---
# REST API

Representational State Transfer - an architectural style for web services.

## Principles

1. **Stateless** - each request contains all information needed
2. **Client-Server** - separation of concerns
3. **Cacheable** - responses can be cached
4. **Uniform Interface** - consistent resource addressing

## Resource Design

Resources are nouns, not verbs:

```
GET    /users          # List users
GET    /users/123      # Get user
POST   /users          # Create user
PUT    /users/123      # Update user
DELETE /users/123      # Delete user
```

## Best Practices

> [!tip] Naming Conventions
> - Use plural nouns (`/users` not `/user`)
> - Use kebab-case (`/user-profiles`)
> - Version your API (`/v1/users`)

See [[wiki/HTTP#Methods]] for method semantics.

## Implementation

Used in [[projects/WebApp]] with [[wiki/Rust]].

#api #design-patterns
