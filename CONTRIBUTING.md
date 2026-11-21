# Contributing to dataviz

Thanks for helping. Bug reports, chart requests, docs fixes and code are all
welcome.

## Reporting bugs and requesting features

Open an [issue](https://github.com/dataviz-rs/dataviz/issues/new/choose).
For a bug, include the smallest code that shows it, what you expected, and
the dataviz version and platform. A screenshot of the wrong chart helps.

## Making a change

1. Fork the repository and branch from `main`.
2. Make the change, with a test when it fixes a bug or adds a feature.
3. Run the same checks as CI:

   ```sh
   cargo fmt --all
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all-features
   ```

   For the Python bindings, see `python/README.md`; for JavaScript,
   `wasm/build.sh`.
4. Add a line under "Unreleased" in `CHANGELOG.md` for user-visible changes.
5. Open a pull request describing what changed and why.

## Compatibility

The `dataviz` crate follows semver from 1.0: public APIs are not removed or
changed incompatibly within 1.x. If a change would break existing code, open
an issue first so we can find a compatible way.

## Code of conduct

By taking part you agree to follow the [code of conduct](CODE_OF_CONDUCT.md).
