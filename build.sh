cargo fmt
cargo clippy
cargo build --release
docker build -t ironstream:latest .
truncate -s 0 build.txt
date --utc >> build.txt
docker inspect --format='{{.Id}}' ironstream:latest >> build.txt
rhash --sha256 -r src Cargo.toml >> build.txt