#!/usr/bin/env bash
cd badam-sat-client
trunk build --release --public-url /badam_sat

cd ../badam-sat-server
cargo build --release

cd ..
if [ -d dist ]; then
    rm -r dist
fi
cp -r badam-sat-client/dist ./
