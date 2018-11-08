## rmdns

Rust mDNS library, with Querier (client) and Responder (server).

**Only for Prototype ** now.

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

Now, client sends a DNS packet by every 5 seconds.
