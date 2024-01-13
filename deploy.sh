#!/usr/bin/env bash

cross build --release --target aarch64-unknown-linux-musl

# copy the binary via rsync to 'genoa' ~/dawdle/dawdle
# and restart the service `sudo systemctl restart dawdle.service`
rsync -avzP target/aarch64-unknown-linux-musl/release/dawdle-server genoa:~/dawdle/dawdle-server
ssh genoa "sudo systemctl restart dawdle.service"
