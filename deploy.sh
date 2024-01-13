#!/usr/bin/env bash

cross build --release --target aarch64-unknown-linux-musl
rsync -avzP target/aarch64-unknown-linux-musl/release/dawdle-server genoa:~/dawdle/dawdle-server
rsync -avzP src/default_home genoa:~/dawdle/.files/
ssh genoa "sudo systemctl restart dawdle.service"
