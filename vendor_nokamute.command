#!/bin/zsh
# Downloads Nokamute's source and its Rust dependencies (cargo vendor) so ThoraxRex
# can be tested against Nokamute in Claude's sandbox. Writes ref/nokamute-vendored.tar.gz
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
mkdir -p ref && cd ref
rm -rf nokamute
git clone --depth 1 https://github.com/edre/nokamute nokamute || exit 1
cd nokamute
mkdir -p .cargo
cargo vendor vendor > .cargo/config.toml || exit 1
cd ..
tar czf nokamute-vendored.tar.gz nokamute
echo "VENDOR DONE" > nokamute-vendor.done
ls -la nokamute-vendored.tar.gz
