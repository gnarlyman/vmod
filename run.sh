#!/bin/bash
# Development run script that sets up the environment for GSettings schema

export GSETTINGS_SCHEMA_DIR=resources
cargo run "$@"
