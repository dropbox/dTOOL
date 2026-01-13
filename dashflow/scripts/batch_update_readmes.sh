#!/bin/bash
set -euo pipefail
# Batch update remaining crate READMEs to minimal format
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

CRATES_DIR="crates"

# Function to create minimal README for a crate
create_minimal_readme() {
    local crate_name=$1
    local crate_type=$2  # llm, vectorstore, tool, search, utility
    local description=$3

    case $crate_type in
        llm)
            cat > "$CRATES_DIR/$crate_name/README.md" << 'EOF'
# CRATE_NAME

DESCRIPTION

## Usage

```rust
use dashflow::language_models::ChatModel;
use dashflow::messages::Message;
use CRATE_IMPORT::CLASS_NAME;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set API_KEY environment variable
    let chat = CLASS_NAME::new()MODEL_CODE;

    let messages = vec![Message::human("EXAMPLE_PROMPT")];
    let result = chat.generate(&messages, None).await?;
    println!("{}", result.generations[0].message.as_text());
    Ok(())
}
```

## Documentation

- **[Golden Path Guide](../../docs/GOLDEN_PATH.md)** - Recommended API patterns
- **[API Reference](https://docs.rs/CRATE_NAME)** - Complete API documentation
- **[Main Repository](../../README.md)** - Full project documentation

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
CRATE_NAME = "1.6"
```
EOF
            ;;
        vectorstore|tool|search|utility)
            cat > "$CRATES_DIR/$crate_name/README.md" << 'EOF'
# CRATE_NAME

DESCRIPTION

## Documentation

- **[Golden Path Guide](../../docs/GOLDEN_PATH.md)** - Recommended API patterns
- **[API Reference](https://docs.rs/CRATE_NAME)** - Complete API documentation
- **[Main Repository](../../README.md)** - Full project documentation

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
CRATE_NAME = "1.6"
```
EOF
            ;;
    esac

    # Replace placeholders
    sed -i '' "s/CRATE_NAME/$crate_name/g" "$CRATES_DIR/$crate_name/README.md"
    sed -i '' "s/DESCRIPTION/$description/g" "$CRATES_DIR/$crate_name/README.md"
}

# Just create a simple utility template for all remaining crates
for crate_dir in $CRATES_DIR/dashflow-*; do
    crate_name=$(basename $crate_dir)

    # Skip already processed crates
    case $crate_name in
        dashflow|dashflow-anthropic|dashflow-openai|dashflow-cohere|dashflow-ollama|dashflow-groq)
            continue
            ;;
    esac

    # Create simple minimal README
    cat > "$crate_dir/README.md" << EOF
# $crate_name

${crate_name#dashflow-} integration for DashFlow Rust.

## Documentation

- **[Golden Path Guide](../../docs/GOLDEN_PATH.md)** - Recommended API patterns
- **[API Reference](https://docs.rs/$crate_name)** - Complete API documentation
- **[Main Repository](../../README.md)** - Full project documentation

## Installation

Add to your \`Cargo.toml\`:
\`\`\`toml
[dependencies]
$crate_name = "1.6"
\`\`\`
EOF

    echo "Updated: $crate_name"
done

# Update dashflow-cli separately (already done but ensure consistency)
echo "Batch update complete!"
