#!/bin/bash
git pull
RUSTFLAGS="-C target-cpu=native" cargo build --release
sudo service ironstream restart