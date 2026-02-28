# RUST_STYLE_GUIDE.md

Last updated: 2026-02-28

This guide defines the required documentation and comment structure for all Rust code in `local-guard`.
It is intentionally documentation-heavy for maintainers with strong `C++`/`Python`/`Pascal` OOP backgrounds and limited Rust experience.

## Core rule

If code is non-trivial, explain it.
Prefer too much useful documentation over too little.

## Mandatory requirements

- Every crate root (`lib.rs` or `main.rs`) must have crate-level `//!` docs.
- Every public API item must have `///` rustdoc.
- Non-obvious private/internal items must also have explanatory rustdoc.
- Complex logic blocks must include inline `//` comments describing intent and invariants.
- Concurrency, ownership, lifetimes, and error paths must be explicitly documented.
- Comments must be kept in sync with code changes.

## Standard documentation structure

Use this exact section order for crate and module docs:

1. Purpose
2. Responsibilities
3. Data flow
4. Ownership and lifetimes
5. Error model
6. Security and privacy notes
7. Example usage (if applicable)

## Crate root template (`src/lib.rs` or `src/main.rs`)

```rust
//! # <crate_name>
//!
//! ## Purpose
//! <What this crate does and why it exists.>
//!
//! ## Responsibilities
//! - <Responsibility 1>
//! - <Responsibility 2>
//!
//! ## Data flow
//! <How data enters, transforms, and leaves this crate.>
//!
//! ## Ownership and lifetimes
//! <How ownership is passed, borrowed, or shared.>
//!
//! ## Error model
//! <Primary error types and how callers should handle them.>
//!
//! ## Security and privacy notes
//! <Sensitive data handling, redaction, or transport guarantees.>
//!
//! ## Example
//! ```rust
//! // Minimal usage example.
//! ```
```

## Module file template (`src/<module>.rs`)

```rust
//! # Module: <module_name>
//!
//! ## Purpose
//! <Why this module exists.>
//!
//! ## Responsibilities
//! - <Responsibility 1>
//! - <Responsibility 2>
//!
//! ## Invariants
//! - <Invariant 1>
//! - <Invariant 2>
//!
//! ## Error model
//! <Error behavior and guarantees.>
//!
//! ## Security and privacy notes
//! <Any sensitive behavior.>
```

## Public type templates

### Struct template

```rust
/// <One-line summary.>
///
/// # Purpose
/// <What problem this type solves.>
///
/// # Invariants
/// - <Invariant 1>
/// - <Invariant 2>
///
/// # Fields
/// <How each field should be interpreted.>
///
/// # Security
/// <Any security/privacy constraints.>
#[derive(Debug, Clone)]
pub struct Example {
    /// <Field purpose and units/format.>
    pub id: String,
}
```

### Enum template

```rust
/// <One-line summary.>
///
/// # Semantics
/// <How variants should be interpreted by callers.>
#[derive(Debug, Clone)]
pub enum ExampleState {
    /// <When this state applies.>
    Idle,
    /// <When this state applies and required caller behavior.>
    Running,
}
```

### Trait template

```rust
/// <One-line summary.>
///
/// # Contract
/// <Behavioral guarantees and obligations for implementers.>
pub trait ExampleService {
    /// <Method summary.>
    ///
    /// # Parameters
    /// - `input`: <meaning>
    ///
    /// # Returns
    /// <Returned value semantics.>
    ///
    /// # Errors
    /// <Failure cases and recovery guidance.>
    fn run(&self, input: &str) -> Result<(), ExampleError>;
}
```

## Function/method template

```rust
/// <One-line summary.>
///
/// # Purpose
/// <What this function does and where it fits in the flow.>
///
/// # Parameters
/// - `arg1`: <meaning, units, accepted range>
/// - `arg2`: <meaning>
///
/// # Returns
/// <What is returned and how to interpret it>
///
/// # Errors
/// Returns an error when:
/// - <condition 1>
/// - <condition 2>
///
/// # Side effects
/// <I/O, network calls, state mutations, logging>
///
/// # Security and privacy
/// <Any sensitive behavior>
///
/// # Example
/// ```rust
/// // Minimal example call.
/// ```
pub fn example(arg1: usize, arg2: &str) -> Result<String, ExampleError> {
    // <Step-level intent comment>
    // <Invariant comment>
    Ok(format!("{arg2}:{arg1}"))
}
```

## Error type template

```rust
/// Error type for <module or feature>.
///
/// # Caller guidance
/// <How callers should branch/retry/fail for each variant category.>
#[derive(Debug, thiserror::Error)]
pub enum ExampleError {
    /// <Transient error; caller may retry with backoff.>
    #[error("temporary failure: {0}")]
    Transient(String),

    /// <Permanent error; caller should not retry without input change.>
    #[error("invalid input: {0}")]
    InvalidInput(String),
}
```

## Inline comment template for complex blocks

Use this pattern immediately above non-obvious code:

```rust
// Why:
// - <business or technical reason>
// Invariant:
// - <what must remain true>
// Failure mode:
// - <what can go wrong and how it is handled>
// Safety:
// - <ownership/concurrency/security note>
```

## State machine documentation template

When implementing states/transitions, include:

- State list and meaning.
- Allowed transitions.
- Invalid transitions and handling.
- Trigger events.
- Recovery path.

Example skeleton:

```rust
/// Auth state machine.
///
/// States:
/// - `Unauthenticated`
/// - `Authenticated`
/// - `ReauthRequired`
///
/// Allowed transitions:
/// - `Unauthenticated -> Authenticated` on login success
/// - `Authenticated -> ReauthRequired` on token expiry
/// - `ReauthRequired -> Authenticated` on reauth success
///
/// Invalid transitions:
/// - `Authenticated -> Unauthenticated` without explicit logout
```

## Test documentation expectations

- Every test module should include a brief doc comment describing what is validated.
- Complex fixtures should include comments explaining setup intent.
- Golden tests should explain update policy for snapshots/fixtures.

## Pull request documentation checklist

Before merging, verify:

- [ ] Crate-level `//!` docs exist and follow standard structure.
- [ ] Public API rustdoc coverage is complete.
- [ ] Non-obvious private logic is documented.
- [ ] Ownership/lifetime/concurrency behavior is explained where relevant.
- [ ] Security/privacy implications are documented.
- [ ] Rustdoc builds with warnings as errors.
- [ ] Doc tests pass (if examples are present).

## Verification commands

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items
cargo test --workspace --doc
```
