#!/bin/bash
# Helper scripts for DashFlow rebranding workers

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function: Rename a single crate
# Usage: rename_crate dashflow-openai dashflow-openai
rename_crate() {
    local old_name=$1
    local new_name=$2

    echo -e "${YELLOW}Renaming $old_name → $new_name${NC}"

    # Check if old crate exists
    if [ ! -d "crates/$old_name" ]; then
        echo -e "${RED}Error: crates/$old_name does not exist${NC}"
        return 1
    fi

    # Rename directory
    echo "  1. Renaming directory..."
    mv "crates/$old_name" "crates/$new_name"

    # Update Cargo.toml
    echo "  2. Updating Cargo.toml..."
    cd "crates/$new_name"

    # Mac-compatible sed
    sed -i '' "s/name = \"$old_name\"/name = \"$new_name\"/" Cargo.toml
    sed -i '' 's/for DashFlow Rust/for DashFlow/g' Cargo.toml
    sed -i '' 's/DashFlow Rust/DashFlow/g' Cargo.toml

    # Update lib.rs
    echo "  3. Updating src/lib.rs..."
    if [ -f "src/lib.rs" ]; then
        sed -i '' 's/DashFlow Rust/DashFlow/g' src/lib.rs
        sed -i '' 's/`DashFlow`/`DashFlow`/g' src/lib.rs
    fi

    # Update README if exists
    if [ -f "README.md" ]; then
        echo "  4. Updating README.md..."
        sed -i '' "s/# $old_name/# $new_name/g" README.md
        sed -i '' 's/DashFlow Rust/DashFlow/g' README.md
    fi

    cd ../..

    # Update workspace Cargo.toml
    echo "  5. Updating workspace Cargo.toml..."
    sed -i '' "s/\"crates\\/$old_name\"/\"crates\\/$new_name\"/" Cargo.toml

    echo -e "${GREEN}✓ Directory renamed${NC}"
}

# Function: Update all Cargo.toml files to reference new crate name
# Usage: update_cargo_references dashflow-openai dashflow-openai
update_cargo_references() {
    local old_name=$1
    local new_name=$2
    local old_lib=$(echo $old_name | tr '-' '_')
    local new_lib=$(echo $new_name | tr '-' '_')

    echo -e "${YELLOW}Updating Cargo.toml references: $old_name → $new_name${NC}"

    # Find all Cargo.toml files
    local count=0
    for file in $(rg -l "$old_name" --type toml 2>/dev/null); do
        sed -i '' "s/$old_name = /$new_name = /g" "$file"
        ((count++))
    done

    echo -e "${GREEN}✓ Updated $count Cargo.toml files${NC}"
}

# Function: Update all Rust imports
# Usage: update_rust_imports dashflow_openai dashflow_openai
update_rust_imports() {
    local old_lib=$1
    local new_lib=$2

    echo -e "${YELLOW}Updating Rust imports: $old_lib → $new_lib${NC}"

    # Find all .rs files
    local count=0
    for file in $(rg -l "use $old_lib" --type rust 2>/dev/null); do
        sed -i '' "s/use $old_lib/use $new_lib/g" "$file"
        sed -i '' "s/$old_lib::/$new_lib::/g" "$file"
        ((count++))
    done

    echo -e "${GREEN}✓ Updated $count Rust files${NC}"
}

# Function: Complete rename of a crate (all steps)
# Usage: complete_rename dashflow-openai dashflow-openai
complete_rename() {
    local old_name=$1
    local new_name=$2
    local old_lib=$(echo $old_name | tr '-' '_')
    local new_lib=$(echo $new_name | tr '-' '_')

    echo -e "${GREEN}═══════════════════════════════════════════════${NC}"
    echo -e "${GREEN}Complete rename: $old_name → $new_name${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════${NC}"

    # Step 1: Rename crate
    rename_crate "$old_name" "$new_name"

    # Step 2: Update Cargo.toml references
    update_cargo_references "$old_name" "$new_name"

    # Step 3: Update Rust imports
    update_rust_imports "$old_lib" "$new_lib"

    # Step 4: Test build
    echo -e "${YELLOW}Testing build...${NC}"
    if cargo build --package "$new_name" 2>&1 | tee "build_$new_name.log"; then
        echo -e "${GREEN}✓ Build successful${NC}"
    else
        echo -e "${RED}✗ Build failed - check build_$new_name.log${NC}"
        return 1
    fi

    # Step 5: Git commit
    echo -e "${YELLOW}Creating git commit...${NC}"
    git add .
    local cargo_count=$(rg -l "$old_name → $new_name" --type toml 2>/dev/null | wc -l | tr -d ' ')
    local rust_count=$(rg -l "$old_lib → $new_lib" --type rust 2>/dev/null | wc -l | tr -d ' ')

    git commit -m "[WORKER] Rename $old_name → $new_name

- Renamed crate directory
- Updated Cargo.toml package name and description
- Updated lib.rs and README documentation
- Updated $cargo_count Cargo.toml dependency references
- Updated $rust_count Rust import statements
- Build verified successful"

    echo -e "${GREEN}✓ Committed${NC}"
}

# Function: Batch rename (process list of crates)
# Usage: batch_rename "dashflow-openai dashflow-openai" "dashflow-anthropic dashflow-anthropic" ...
batch_rename() {
    local batch_name=$1
    shift
    local crates=("$@")

    echo -e "${GREEN}═══════════════════════════════════════════════${NC}"
    echo -e "${GREEN}Starting batch: $batch_name${NC}"
    echo -e "${GREEN}Crates to rename: ${#crates[@]}${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════${NC}"

    local success=0
    local failed=0

    for pair in "${crates[@]}"; do
        local old=$(echo $pair | cut -d' ' -f1)
        local new=$(echo $pair | cut -d' ' -f2)

        if complete_rename "$old" "$new"; then
            ((success++))
        else
            ((failed++))
            echo -e "${RED}Failed to rename $old, stopping batch${NC}"
            return 1
        fi
    done

    echo -e "${GREEN}═══════════════════════════════════════════════${NC}"
    echo -e "${GREEN}Batch complete: $batch_name${NC}"
    echo -e "${GREEN}Success: $success, Failed: $failed${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════${NC}"

    # Test workspace build
    echo -e "${YELLOW}Testing workspace build...${NC}"
    if cargo build --workspace 2>&1 | tee "build_batch_$batch_name.log"; then
        echo -e "${GREEN}✓ Workspace build successful${NC}"

        # Create batch summary commit
        git commit --allow-empty -m "[MANAGER] Batch complete: $batch_name

Renamed $success crates successfully
Workspace builds successfully

Next: Continue to next batch"
    else
        echo -e "${RED}✗ Workspace build failed - check build_batch_$batch_name.log${NC}"
        return 1
    fi
}

# Function: Check for remaining dashflow references
check_remaining() {
    echo -e "${YELLOW}Checking for remaining dashflow/dashflow references...${NC}"

    echo "=== Rust files ==="
    rg -i "dashflow|dashflow" --type rust | head -20

    echo ""
    echo "=== TOML files ==="
    rg -i "dashflow|dashflow" --type toml | head -20

    echo ""
    echo "=== Markdown files ==="
    rg -i "dashflow|dashflow" --type md | head -20
}

# Function: Test build status
test_build() {
    echo -e "${YELLOW}Testing workspace build...${NC}"
    if cargo build --workspace 2>&1 | tee build_test.log; then
        echo -e "${GREEN}✓ Build successful${NC}"
        return 0
    else
        echo -e "${RED}✗ Build failed${NC}"
        echo "Last 20 errors:"
        tail -20 build_test.log
        return 1
    fi
}

# Export functions
export -f rename_crate
export -f update_cargo_references
export -f update_rust_imports
export -f complete_rename
export -f batch_rename
export -f check_remaining
export -f test_build

echo -e "${GREEN}Rebranding helper functions loaded!${NC}"
echo ""
echo "Available functions:"
echo "  rename_crate OLD NEW              - Rename crate directory and update its files"
echo "  update_cargo_references OLD NEW   - Update Cargo.toml dependencies"
echo "  update_rust_imports OLD NEW       - Update Rust imports"
echo "  complete_rename OLD NEW           - Complete rename (all steps + commit)"
echo "  batch_rename NAME \"OLD NEW\" ...   - Batch process multiple crates"
echo "  check_remaining                   - Check for remaining dashflow references"
echo "  test_build                        - Test workspace build"
echo ""
echo "Example usage:"
echo "  complete_rename dashflow-openai dashflow-openai"
echo "  batch_rename \"Batch1\" \"dashflow-openai dashflow-openai\" \"dashflow-anthropic dashflow-anthropic\""
