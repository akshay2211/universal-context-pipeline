# Rust error handling — working notes

Quick notes for future me on the patterns I keep reaching for.

## `Result<T, E>` is the spine

`Result<T, E>` is just an enum with `Ok(T)` and `Err(E)`. Idiomatic Rust returns `Result` from anywhere a fallible operation lives — the type system enforces that callers acknowledge the error path. Compare to exceptions in other languages: nothing is hidden. If a function returns `Result`, the compiler will yell at you if you ignore it.

## The `?` operator

`?` is the workhorse. Inside a function returning `Result`, `expr?` means "unwrap the Ok value or propagate the Err immediately." It desugars to a match plus a `From` conversion on the error type. The key thing: it only works in functions that themselves return `Result` (or `Option`, with caveats).

Example:

```rust
fn read_config() -> Result<Config, anyhow::Error> {
    let bytes = std::fs::read(path)?;          // io::Error → anyhow::Error
    let config: Config = toml::from_slice(&bytes)?;   // toml::Error → anyhow::Error
    Ok(config)
}
```

The `?` collapses three lines per call site into one. Without it, error-prone manual matching everywhere.

## anyhow vs thiserror

Two libraries cover almost all real-world cases:

- **anyhow** — for application code. `anyhow::Result<T>` is `Result<T, anyhow::Error>`. Erases the concrete error type; great when you don't care about programmatic recovery, only logging and surfacing.
- **thiserror** — for library code. Lets you derive `Error` cleanly on your own typed errors. Use when callers might want to match on the variant.

Rule of thumb: binaries use anyhow, libraries use thiserror. Mixed crates can use both.

## `.context()` is underrated

`anyhow::Context` lets you attach a description to any error:

```rust
let bytes = std::fs::read(&path)
    .with_context(|| format!("reading config at {}", path.display()))?;
```

Now the error chain shows *what you were doing* when it failed, not just "no such file or directory." This pays for itself the first time a user submits a bug report.

## Don't use `unwrap()` outside tests and main

`unwrap()` is fine in tests, examples, and the top-level main of a small CLI. Anywhere else it's a footgun: the program panics with no context, the user sees a stack trace they can't act on. Replace with `?` and bubble up.

## TODO

- Look at `eyre` / `color-eyre` for nicer error reports in CLIs.
- Read the std::error::Error refactor RFC again — Source/Backtrace shape is changing.
