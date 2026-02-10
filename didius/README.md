# Didius 

### Order handler
This code is AI-generated.

### Tests
```
cargo run --example hantoo_check
python3 tests/verify_rust_oms.py
cargo build --release
cargo test --test logger_test
cargo test --test oms_hantoo_ngt_futopt -- --nocapture
RUSTFLAGS="-L /usr/lib/x86_64-linux-gnu -l python3.13"  cargo test --test oms_hantoo_ngt_futopt -- --nocapture
```

### Python 
```
nix-shell
maturin build
exit
nix-shell
```