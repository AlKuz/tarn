---
tags:
  - project/webapp
  - development
  - database
created: 2024-01-13
---
# Database Schema

PostgreSQL schema for [[projects/WebApp]].

## Tables

### users

| Column | Type | Description |
|--------|------|-------------|
| id | UUID | Primary key |
| email | VARCHAR | Unique email |
| created_at | TIMESTAMP | Creation time |

### sessions

| Column | Type | Description |
|--------|------|-------------|
| id | UUID | Primary key |
| user_id | UUID | Foreign key |
| expires_at | TIMESTAMP | Expiration |

## Migrations

Managed with SQLx migrations.

Related: [[projects/webapp/development/API Design]]

#sql #postgres
