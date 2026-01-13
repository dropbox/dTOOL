#!/bin/bash
# Run TLC model checking for all TLA+ specs in docs/tlaplus.
#
# Environment:
#   SKIP_TLAPLUS=true        Skip checks (exit 0)
#   TLA2TOOLS_JAR=...        Path to tla2tools.jar (overrides auto-discovery)
#   TLA2TOOLS_URL=...        URL to download tla2tools.jar from (default: GitHub latest release)
#   TLA_AUTO_DOWNLOAD=false  Disable auto-download of tla2tools.jar (default: true)
#   JAVA_BIN=...             Path to a java binary (overrides auto-discovery)
#   TLAPLUS_SPECS="A,B"      Only run these specs (base names without .tla)
#   TLC_WORKERS=1            TLC workers (default: 1)
#   TLC_JAVA_OPTS="..."      Extra JVM opts (default: empty)
#   TLC_EXTRA_ARGS="..."     Extra args passed to tlc2.TLC (default: empty)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ "${SKIP_TLAPLUS:-false}" = "true" ]; then
    echo "SKIP_TLAPLUS=true; skipping TLA+ checks."
    exit 0
fi

SPEC_DIR="$REPO_ROOT/docs/tlaplus"
if [ ! -d "$SPEC_DIR" ]; then
    echo "Error: Missing docs/tlaplus directory."
    exit 1
fi

find_java() {
    if [ -n "${JAVA_BIN:-}" ] && [ -x "${JAVA_BIN:-}" ]; then
        echo "$JAVA_BIN"
        return 0
    fi

    if command -v java >/dev/null 2>&1; then
        if java -version >/dev/null 2>&1; then
            command -v java
            return 0
        fi
    fi

    if command -v brew >/dev/null 2>&1; then
        local prefix
        prefix="$(brew --prefix openjdk 2>/dev/null || true)"
        if [ -n "$prefix" ] && [ -x "$prefix/bin/java" ]; then
            if "$prefix/bin/java" -version >/dev/null 2>&1; then
                echo "$prefix/bin/java"
                return 0
            fi
        fi
    fi

    return 1
}

download_tla_jar() {
    local dest="$1"
    local url="${TLA2TOOLS_URL:-https://github.com/tlaplus/tlaplus/releases/latest/download/tla2tools.jar}"

    if ! command -v curl >/dev/null 2>&1; then
        echo "Error: curl not found; cannot auto-download tla2tools.jar."
        return 1
    fi

    mkdir -p "$(dirname "$dest")"

    local tmp
    tmp="$(mktemp "${dest}.tmp.XXXXXX")"
    if ! curl -fsSL "$url" -o "$tmp"; then
        rm -f "$tmp"
        echo "Error: Failed to download tla2tools.jar from: $url"
        return 1
    fi

    mv "$tmp" "$dest"
    return 0
}

find_tla_jar() {
    if [ -n "${TLA2TOOLS_JAR:-}" ] && [ -f "${TLA2TOOLS_JAR:-}" ]; then
        echo "$TLA2TOOLS_JAR"
        return 0
    fi

    local cached="$REPO_ROOT/target/tlaplus/tla2tools.jar"
    if [ -f "$cached" ]; then
        echo "$cached"
        return 0
    fi

    local toolbox_jar="/Applications/TLA+ Toolbox.app/Contents/Eclipse/tla2tools.jar"
    if [ -f "$toolbox_jar" ]; then
        echo "$toolbox_jar"
        return 0
    fi

    if command -v brew >/dev/null 2>&1; then
        local prefix
        prefix="$(brew --prefix tlaplus 2>/dev/null || true)"
        if [ -n "$prefix" ] && [ -d "$prefix" ]; then
            local jar
            jar="$(find "$prefix" -maxdepth 5 -type f -name 'tla2tools.jar' 2>/dev/null | head -1 || true)"
            if [ -n "$jar" ] && [ -f "$jar" ]; then
                echo "$jar"
                return 0
            fi
        fi
    fi

    if [ "${TLA_AUTO_DOWNLOAD:-true}" != "false" ]; then
        if download_tla_jar "$cached"; then
            echo "$cached"
            return 0
        fi
    fi

    return 1
}

TLA_JAR="$(find_tla_jar || true)"
if [ -z "$TLA_JAR" ]; then
    echo "Error: Could not find tla2tools.jar."
    echo ""
    echo "Install tools (macOS/Homebrew):"
    echo "  brew install --cask tla+-toolbox"
    echo ""
    echo "Or set:"
    echo "  export TLA2TOOLS_JAR=/path/to/tla2tools.jar"
    echo ""
    echo "Or allow auto-download (default):"
    echo "  unset TLA_AUTO_DOWNLOAD"
    exit 1
fi

JAVA="$(find_java || true)"
if [ -z "$JAVA" ]; then
    echo "Error: Could not find a working java runtime (required to run TLC)."
    echo ""
    echo "If you have Homebrew OpenJDK installed, try:"
    echo "  export JAVA_BIN=\"$(brew --prefix openjdk 2>/dev/null)/bin/java\""
    echo ""
    echo "Or ensure a working 'java' is on PATH."
    exit 1
fi

WORKERS="${TLC_WORKERS:-1}"
if [ -z "${TLC_JAVA_OPTS:-}" ]; then
    JAVA_OPTS="-XX:+UseParallelGC"
else
    JAVA_OPTS="$TLC_JAVA_OPTS"
fi
EXTRA_ARGS="${TLC_EXTRA_ARGS:-}"

echo "=== TLA+ TLC Model Checking ==="
echo "Jar: $TLA_JAR"
echo "Java: $JAVA"
echo "Specs: $SPEC_DIR"
echo ""

pushd "$SPEC_DIR" >/dev/null

ONLY_SPECS=()
if [ -n "${TLAPLUS_SPECS:-}" ]; then
    IFS=',' read -r -a _requested <<< "$TLAPLUS_SPECS"
    for spec in "${_requested[@]}"; do
        spec="${spec//[[:space:]]/}"
        if [ -n "$spec" ]; then
            ONLY_SPECS+=( "$spec" )
        fi
    done
fi

spec_is_requested() {
    local requested
    local candidate="$1"

    if [ "${#ONLY_SPECS[@]}" -eq 0 ]; then
        return 0
    fi

    for requested in "${ONLY_SPECS[@]}"; do
        if [ "$requested" = "$candidate" ]; then
            return 0
        fi
    done

    return 1
}

SPECS=( *.tla )
if [ "${#SPECS[@]}" -eq 1 ] && [ "${SPECS[0]}" = "*.tla" ]; then
    echo "Error: No *.tla files found in $SPEC_DIR."
    exit 1
fi

for spec_file in "${SPECS[@]}"; do
    base="${spec_file%.tla}"

    # Skip MC_* wrapper modules - they'll be used when we encounter their parent spec
    if [[ "$base" == MC_* ]]; then
        continue
    fi
    if ! spec_is_requested "$base"; then
        continue
    fi

    # Check if an MC wrapper exists for this spec (for specs with set/tuple constants)
    mc_spec="MC_${base}.tla"
    mc_cfg="MC_${base}.cfg"
    if [ -f "$mc_spec" ] && [ -f "$mc_cfg" ]; then
        # Use MC wrapper which has TLC-compatible constant definitions
        run_spec="$mc_spec"
        run_cfg="$mc_cfg"
        run_base="MC_${base}"
        echo "--- $spec_file (using $run_spec / $run_cfg) ---"
    else
        # Use original spec directly
        run_spec="$spec_file"
        run_cfg="${base}.cfg"
        run_base="$base"
        if [ ! -f "$run_cfg" ]; then
            echo "Error: Missing config file for $spec_file: expected $run_cfg"
            exit 1
        fi
        echo "--- $spec_file ($run_cfg) ---"
    fi

    meta_dir="$REPO_ROOT/target/tlaplus/$run_base"
    mkdir -p "$meta_dir"

    # TLC writes state/fingerprint data under -metadir to avoid polluting the repo.
    "$JAVA" $JAVA_OPTS -cp "$TLA_JAR" tlc2.TLC \
        -deadlock \
        -workers "$WORKERS" \
        -metadir "$meta_dir" \
        -config "$run_cfg" \
        $EXTRA_ARGS \
        "$run_spec"
    echo ""
done

popd >/dev/null

echo "All TLA+ checks passed."
