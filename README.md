# Logging

`mini-redis` uses [`tracing`](https://github.com/tokio-rs/tracing) to provide structured logs. Debug logs can be displayed by running:

```console
RUST_LOG=debug cargo run --bin server
```

Logs will appear when commands are sent to the server from a client.

More documentation on enabling different log levels and filtering logs can be found [here](https://docs.rs/env_logger/0.7.1/env_logger/#enabling-logging).
