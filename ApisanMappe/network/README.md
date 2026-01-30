# Rust Networking for Elevator project

To use this library add the following to `Cargo.toml`:

```toml
[dependencies]
network-rust = { git = "https://github.com/TTK4145/network-rust", tag = "v0.X.0" }
```

For most recent release see [releases](https://github.com/TTK4145/network-rust/releases). Note
that we will come with breaking changes to `master`, so depending on the `master` branch directly
might lead to some issues.

When using the library in your project, it will be available under the
`network_rust` namespace, example:

```rust
use network_rust::udpnet;

// do stuff with udpnet::peers::tx(), or similar
```

For an example of usage, see [main.rs](src/main.rs).


## Logging

This library uses the standard `log` crate to log errors whenever failing to send messages fail. These messages are **ignored by default**,
if you have configured a logging setup (through e.g. `env_logger`), you can easily change the `RUST_LOG` environment variable to ignore
the logging from this module. E.g. if your default logging level is trace you would do the following in most shells:

```sh
export RUST_LOG=trace,network_rust=off
```

This is only relevant if you have set up logging for your project.

