#!/bin/bash

set -e


# echo "📦 Rebuilding and restarting Docker containers..."
# export IMAGE_TAG=$(openssl rand -base64 6 | sha1 )

# docker build -t bigo:${IMAGE_TAG} .

# # docker compose down

# docker compose up -d

# docker logs bigo-rust-server-2-1 -f


#!/bin/bash

# Set the IMAGE_TAG variable if it's passed as an argument, otherwise use default
# IMAGE_TAG=${1:-"latest"}
# echo "Using image tag: $IMAGE_TAG"

cargo build --release

# Define the list of all nodes
ALL_NODE_IPS="127.0.0.1:8001,127.0.0.1:8102,127.0.0.1:8203,127.0.0.1:8304,127.0.0.1:8405,127.0.0.1:8506,127.0.0.1:8607"

# Kill any previously running instances
# echo "Stopping any previously running instances..."
# # pkill -f "rs" || true
# sleep 2

# Function to start a single server instance
start_server() {
  local server_num=$1
  local http_port=$2
  
  # Set environment variables
  export RUST_LOG=info
  export RUST_BACKTRACE=1
  export CURRENT_NODE_IP="127.0.0.1:$http_port"
  export ALL_NODE_IPS="$ALL_NODE_IPS"
  
  echo "Starting rust-server-$server_num on HTTP port $http_port"

  ./target/release/rs > "server-$server_num.log" 2>&1 &
  
  # Store the PID for potential cleanup later
  echo $! > "server-$server_num.pid"
}

stop_all_servers() {
  echo "Stopping all servers..."
  ps aux | grep release | grep -v grep | awk '{ print $2 }' | xargs kill -9
}

# stop_all_servers;

# Make sure the binary is compiled and ready
echo "Compiling the Rust project..."
cargo build --release

stop_all_servers;

sleep 5;

# Start all server instances
start_server 1 8001
start_server 2 8102
start_server 3 8203
start_server 4 8304
start_server 5 8405
start_server 6 8506
start_server 7 8607

echo "All servers started. Check individual log files for details."
echo "To stop all servers: pkill -f 'rs'"
