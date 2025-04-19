#!/bin/bash

set -e


echo "📦 Rebuilding and restarting Docker containers..."
export IMAGE_TAG=$(openssl rand -base64 6 | sha1 )

docker build -t bigo:${IMAGE_TAG} .

# docker compose down

docker compose up -d

docker logs bigo-rust-server-2-1 -f
