#!/bin/bash
set -euo pipefail

echo "=== Deprecation Plan Verification (v1.9.0) ==="
echo ""
echo "Phase 1: Deprecation Annotations Status"
echo "========================================"
echo ""
echo "Provider with_tools() methods:"

find_chat_models_file() {
  local crate="$1"
  local base="crates/dashflow-$crate/src"

  if [ -f "$base/chat_models.rs" ]; then
    echo "$base/chat_models.rs"
    return 0
  fi

  if [ -f "$base/chat_models/mod.rs" ]; then
    echo "$base/chat_models/mod.rs"
    return 0
  fi

  return 1
}

with_tools_deprecated_status() {
  local file="$1"
  awk '
    BEGIN { pending_deprecated=0; found=0; bad=0 }
    /^[[:space:]]*#\[deprecated/ { pending_deprecated=1 }
    /^[[:space:]]*(pub[[:space:]]+)?fn[[:space:]]+with_tools[[:space:]]*\(/ {
      found=1
      if (pending_deprecated == 0) { bad=1 }
      pending_deprecated=0
      next
    }
    /^[[:space:]]*(pub[[:space:]]+)?fn[[:space:]]+/ { pending_deprecated=0 }
    /^[[:space:]]*(pub[[:space:]]+)?(struct|enum|trait)[[:space:]]+/ { pending_deprecated=0 }
    END {
      if (found == 0) { exit 2 }
      if (bad == 1) { exit 1 }
      exit 0
    }
  ' "$file"
}

for crate in openai anthropic groq fireworks mistral ollama replicate together xai azure-openai bedrock; do
  if file="$(find_chat_models_file "$crate")"; then
    if with_tools_deprecated_status "$file"; then
      echo "  ✓ dashflow-$crate: DEPRECATED"
    else
      status=$?
      if [ "$status" -eq 2 ]; then
        echo "  - dashflow-$crate: no with_tools() method"
      else
        echo "  ✗ dashflow-$crate: MISSING deprecation"
      fi
    fi
  else
    echo "  - dashflow-$crate: file not found"
  fi
done

echo ""
echo "AgentExecutor deprecations:"

find_agent_executor_file() {
  if [ -f "crates/dashflow/src/core/agents.rs" ]; then
    echo "crates/dashflow/src/core/agents.rs"
    return 0
  fi
  if [ -f "crates/dashflow/src/core/agents/executor.rs" ]; then
    echo "crates/dashflow/src/core/agents/executor.rs"
    return 0
  fi
  return 1
}

item_deprecated_status() {
  local file="$1"
  local item_regex="$2"
  awk -v item_regex="$item_regex" '
    BEGIN { pending_deprecated=0; found=0; result=1 }
    /^[[:space:]]*#\[deprecated/ { pending_deprecated=1 }
    $0 ~ item_regex {
      found=1
      if (pending_deprecated == 1) { result=0 } else { result=1 }
      exit
    }
    /^[[:space:]]*(pub[[:space:]]+)?(struct|enum|trait|fn)[[:space:]]+/ { pending_deprecated=0 }
    END {
      if (found == 0) { result=2 }
      exit result
    }
  ' "$file"
}

if agent_file="$(find_agent_executor_file)"; then
  if item_deprecated_status "$agent_file" '^[[:space:]]*pub[[:space:]]+struct[[:space:]]+AgentExecutor([^A-Za-z0-9_]|$)'; then
    echo "  ✓ AgentExecutor: DEPRECATED"
  else
    status=$?
    if [ "$status" -eq 2 ]; then
      echo "  - AgentExecutor: not found"
    else
      echo "  ✗ AgentExecutor: MISSING deprecation"
    fi
  fi

  if item_deprecated_status "$agent_file" '^[[:space:]]*pub[[:space:]]+struct[[:space:]]+AgentExecutorConfig([^A-Za-z0-9_]|$)'; then
    echo "  ✓ AgentExecutorConfig: DEPRECATED"
  else
    status=$?
    if [ "$status" -eq 2 ]; then
      echo "  - AgentExecutorConfig: not found"
    else
      echo "  ✗ AgentExecutorConfig: MISSING deprecation"
    fi
  fi
else
  echo "  - AgentExecutor: file not found"
  echo "  - AgentExecutorConfig: file not found"
fi

echo ""
echo "Phase 2: Example Updates Status"
echo "================================"
echo ""
echo "Examples using with_tools():"

example_count=0
allowed_count=0

has_allow_deprecated() {
  local file="$1"
  awk '
    BEGIN { found=0; in_allow=0 }
    /^[[:space:]]*#!?\[allow/ { in_allow=1 }
    in_allow == 1 && /deprecated/ { found=1 }
    in_allow == 1 && /\]/ { in_allow=0 }
    END { exit found ? 0 : 1 }
  ' "$file"
}

for file in crates/*/examples/*.rs; do
  if [ -f "$file" ] && grep -q '\.with_tools(' "$file"; then
    example_count=$((example_count + 1))
    basename_file=$(basename "$file")
    if has_allow_deprecated "$file"; then
      echo "  ✓ $basename_file: has #[allow(deprecated)]"
      allowed_count=$((allowed_count + 1))
    else
      echo "  ✗ $basename_file: MISSING #[allow(deprecated)]"
    fi
  fi
done

echo ""
echo "Integration tests using with_tools():"

test_count=0
test_allowed_count=0

while IFS= read -r -d '' tests_dir; do
  while IFS= read -r -d '' file; do
    if grep -q '\.with_tools(' "$file"; then
      test_count=$((test_count + 1))
      relpath=$(echo "$file" | sed 's|crates/||')
      if has_allow_deprecated "$file"; then
        echo "  ✓ $relpath"
        test_allowed_count=$((test_allowed_count + 1))
      else
        echo "  ✗ $relpath: MISSING #[allow(deprecated)]"
      fi
    fi
  done < <(find "$tests_dir" -type f -name '*.rs' -print0)
done < <(find crates -type d -name tests -print0)

echo ""
echo "Summary"
echo "======="
echo "Examples with with_tools(): $example_count"
echo "Examples with #[allow(deprecated)]: $allowed_count"
echo "Tests with with_tools(): $test_count"
echo "Tests with #[allow(deprecated)]: $test_allowed_count"
echo ""

if [ $example_count -eq $allowed_count ] && [ $test_count -eq $test_allowed_count ]; then
  echo "✅ Phase 2 COMPLETE: All examples and tests properly annotated"
else
  echo "⚠️  Phase 2 INCOMPLETE: Some files need #[allow(deprecated)]"
fi
