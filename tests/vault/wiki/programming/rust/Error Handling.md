---
tags:
  - programming/rust
  - patterns
created: 2024-01-10
---
# Error Handling

Error handling patterns in [[wiki/Rust]].

## Result Type

```rust
fn divide(a: i32, b: i32) -> Result<i32, String> {
    if b == 0 {
        Err("division by zero".into())
    } else {
        Ok(a / b)
    }
}
```

## The ? Operator

Propagate errors concisely:

```rust
fn process() -> Result<(), Error> {
    let data = read_file()?;
    let parsed = parse_data(&data)?;
    save_result(parsed)?;
    Ok(())
}
```

## Custom Error Types

Use `thiserror` for ergonomic error definitions.

Related: [[wiki/programming/rust/Async Rust]]

#error-handling #thiserror
