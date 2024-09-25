#!/usr/bin/env bash

cargo zigbuild --release --target aarch64-unknown-linux-musl
rsync -avzP target/aarch64-unknown-linux-musl/release/dawdle-server genoa:~/dawdle/dawdle-server
rsync -avzP src/default-home genoa:~/dawdle/users/
ssh genoa "sudo systemctl restart dawdle.service"
