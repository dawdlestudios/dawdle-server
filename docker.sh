#!/bin/env bash
docker build --squash --platform linux/amd64,linux/arm64 -t ghcr.io/dawdlestudios/container:latest -f ./docker/user/Dockerfile ./docker/user --push
