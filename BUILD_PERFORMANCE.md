# Faster Windows builds

My Tools uses GPUI Kit and native Windows libraries; compiling the entire Rust
dependency graph from scratch on a GitHub-hosted Windows runner is expensive.

## Measured baseline (GitHub Actions, v0.4.0)

| Job / step | Wall time |
| --- | --- |
| Tagged-release test and build | 18 min 47 sec |
| CI Win32 Rust module tests | 1 min 29 sec |
| CI desktop `cargo check` followed by release build | 17 min 54 sec |
| Release packaging + publish | approximately 11 seconds |

The expensive part is Rust compilation and linking, not the release upload.

## Optimizations applied

1. **Shared dependency cache:** both `.github/workflows/ci.yml` and
   `.github/workflows/release.yml` use `Swatinem/rust-cache@v2`, with a matching
   `shared-key` and two independent workspaces (`rust/target` and
   `desktop/target`). Only CI on `main` saves updates to the cache; tagged
   releases restore it. The cache tracks the Rust compiler version and Cargo
   dependency manifests, and GitHub may evict it when storage limits are hit.
2. **Avoid compiling twice:** CI no longer runs `cargo check` on the GPUI hub
   before `cargo build --release`. The release build already checks types.
   Developers can still use `cargo check` locally for quick feedback.
3. **Faster release link:** `desktop/Cargo.toml` disables cross-crate LTO and
   uses 16 codegen units. This prioritizes build speed for a Windows utility,
   though it can produce a larger EXE and marginally different runtime
   performance. Optimized codegen and symbol stripping remain enabled.

## What to expect

- The **first** run after changing the release profile or Rust toolchain will
  still compile dependencies. It also populates the cache for the next run.
- **Subsequent CI runs and tagged releases** on compatible toolchains should
  reuse compiled dependencies, usually saving considerably more time than
  the small packaging step could.
- A concurrent tag build may start before CI has finished writing a new cache;
  in that case both runs can be cold. Future builds should benefit.
- No target runtime libraries were removed, and the app still ships a single
  `my-tools.exe` from the tagged-release workflow.

To see actual durations and cache results, open the relevant GitHub Actions job,
expand **Restore Rust dependency cache**, and compare the **Build My Tools**
step across runs. Cache hit/miss logs are provided by the action. Do not claim
a particular speedup until comparing completed workflows.
