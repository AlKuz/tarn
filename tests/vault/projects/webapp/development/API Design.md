---
tags:
  - project/webapp
  - development
  - api
created: 2024-01-13
---
# API Design

Backend API design for [[projects/WebApp]].

## Endpoints

### Authentication

```
POST /api/auth/login
POST /api/auth/register
POST /api/auth/refresh
```

### Users

```
GET    /api/users
GET    /api/users/:id
PUT    /api/users/:id
DELETE /api/users/:id
```

## Implementation

Using [[wiki/REST API]] principles with [[wiki/Rust]] and Axum.

See [[projects/webapp/development/Database Schema]] for data models.

#backend #architecture
