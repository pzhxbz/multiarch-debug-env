#!/bin/bash

cd multiarch-debug && cargo build --release
cd .. && mv multiarch-debug/target/release/multiarch-debug ./multiarch_debug
