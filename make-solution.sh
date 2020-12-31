#!/bin/bash
echo "Installing protobuf-compiler"
sudo apt install protobuf-compiler -y -s
if not $which cargo
then
  echo "Downloading and installing Rust Please follow on-screen instructions"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
fi
echo "Compiling this massive app, go take some tea"
RUSTFLAGS="-C target-cpu=native" cargo build --release
echo "Copying file to correct path ie "
pwd
cp ./target/release/tgnews ./
echo "cleaning files"
cargo clean
echo "Done"