## rmdns

Rust mDNS library, with Querier (client) and Responsder (server).

### Usage

run server
```
cargo run --bin responder

```

open another console, run client:
```
cargo run --bin querier
```

see console outputs.

### Explanation 

Now, client send a DNS packet every 5 seconds.
