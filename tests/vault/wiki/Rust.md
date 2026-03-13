---
aliases:
  - Rust Language
  - Rust Programming
tags:
  - programming/rust
  - learning
created: 2024-01-10
---
# Rust

A systems programming language focused on safety and performance.

## Ownership

Every value has exactly one owner. ^ownership-rule

> [!important] The Ownership Rules
> 1. Each value has one owner
> 2. Only one owner at a time
> 3. Value dropped when owner goes out of scope

### Borrowing

References allow borrowing without ownership transfer.

```rust
fn main() {
    let s = String::from("hello");
    let len = calculate_length(&s); // #not-a-tag in code
    println!("Length: {len}");
}

fn calculate_length(s: &str) -> usize {
    s.len()
}
```

The complexity is $O(1)$ for this operation.[^1]

## Lifetimes

Lifetimes ensure references are valid. See [[#Borrowing]] for context.

%%TODO: Add more examples%%

The lifetime annotation syntax:

$$
\text{fn longest<'a>(x: \&'a str, y: \&'a str) -> \&'a str}
$$

## Resources

- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- <https://crates.io>
- Contact: <rustacean@example.com>

#rust/advanced #memory-safety

---

Related: [[projects/WebApp]], [[Home]]

[^1]: String length is stored, not computed.
