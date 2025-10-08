#!/bin/bash

# Initialize TriviaChain Contract
# This script calls the initialize() function on the deployed contract

set -e

# Load environment variables
if [ -f ../.env ]; then
  source ../.env
fi

# Contract address
# CONTRACT_ADDRESS="0x4488af2dd81ea4100f97588aaf5dbf4ec32d8aa2"
CONTRACT_ADDRESS="0x49c90b349fba199c4be542d225d1783ac8c0ddde"

# Arbitrum Sepolia RPC URL
RPC_URL="https://sepolia-rollup.arbitrum.io/rpc"

echo "=========================================="
echo "Initializing TriviaChain Contract"
echo "=========================================="
echo "Contract Address: $CONTRACT_ADDRESS"
echo "Network: Arbitrum Sepolia"
echo ""

# Check if private key is set
if [ -z "$SEPOLIA_PRIVATE_KEY" ]; then
  echo "Error: SEPOLIA_PRIVATE_KEY not found in .env file"
  echo "Please set SEPOLIA_PRIVATE_KEY in your .env file"
  exit 1
fi

# Add 0x prefix if not present
if [[ ! "$SEPOLIA_PRIVATE_KEY" =~ ^0x ]]; then
  SEPOLIA_PRIVATE_KEY="0x$SEPOLIA_PRIVATE_KEY"
fi

echo "Sending initialize transaction..."
echo ""

# Call initialize function using cast
cast send $CONTRACT_ADDRESS \
  "initialize()" \
  --rpc-url $RPC_URL \
  --private-key $SEPOLIA_PRIVATE_KEY \
  --legacy

echo ""
echo "=========================================="
echo "Contract initialized successfully!"
echo "=========================================="
