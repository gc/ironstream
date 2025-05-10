RUSTFLAGS="-C target-cpu=native" cargo build --release
du -h target/release/ironstream-bin target/release/ironstream-bin.exe
