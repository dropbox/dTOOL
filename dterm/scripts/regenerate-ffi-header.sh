#!/bin/bash
# Regenerate FFI header from Rust code
#
# CRITICAL: Run this script after ANY change to FFI structs in:
#   crates/dterm-core/src/ffi/mod.rs
#
# Failure to regenerate causes memory corruption and ASan crashes in DashTerm2.

set -e

cd "$(dirname "$0")/.."

echo "Regenerating FFI header..."
cd crates/dterm-core
cbindgen -c cbindgen.toml -o include/dterm.h

echo "Done. Header written to: crates/dterm-core/include/dterm.h"
echo ""
echo "Next steps:"
echo "  1. git add crates/dterm-core/include/dterm.h"
echo "  2. git commit -m 'fix: regenerate FFI header'"
echo "  3. Copy header to DashTerm2: cp include/dterm.h ~/dashterm2/DTermCore/include/"
