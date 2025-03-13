cargo fmt
cargo clippy
cargo build --release
docker build -t gc261/ironstream:latest .
truncate -s 0 build.txt
date --utc >> build.txt
docker inspect --format='{{.Id}}' gc261/ironstream:latest >> build.txt
# docker push gc261/ironstream:latest