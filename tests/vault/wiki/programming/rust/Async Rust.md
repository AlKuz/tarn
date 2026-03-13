---
tags:
  - programming/rust
  - async
created: 2024-01-10
---
# Async Rust

Asynchronous programming in [[wiki/Rust]].

## Basics

```rust
async fn fetch_data() -> Result<Data, Error> {
    let response = client.get(url).await?;
    response.json().await
}
```

## Tokio Runtime

The most popular async runtime for Rust.

```rust
#[tokio::main]
async fn main() {
    let result = fetch_data().await;
}
```

## Related

- [[wiki/Rust#Ownership]] - ownership in async contexts
- [[wiki/programming/rust/Error Handling]]

#concurrency #tokio
