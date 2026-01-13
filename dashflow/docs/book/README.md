# DashFlow Documentation

This directory contains the source for the DashFlow documentation book, built with [mdBook](https://rust-lang.github.io/mdBook/).

## Building the Documentation

### Prerequisites

Install mdbook:

```bash
cargo install mdbook
```

### Build and Serve Locally

```bash
cd docs/book
mdbook serve --open
```

This will:
1. Build the documentation
2. Start a local web server (default: http://localhost:3000)
3. Open the documentation in your browser
4. Watch for changes and auto-reload

### Build Static Site

```bash
cd docs/book
mdbook build
```

Output will be in `docs/book/book/` directory.

## Structure

```
docs/book/
├── book.toml              # mdBook configuration
├── src/                   # Source markdown files
│   ├── SUMMARY.md         # Table of contents
│   ├── introduction.md    # Introduction page
│   ├── getting-started/   # Getting started guides
│   ├── architecture/      # Architecture documentation
│   ├── core/              # Core features documentation
│   ├── chains/            # Chains documentation
│   ├── agents/            # Agents documentation
│   ├── tools/             # Tools documentation
│   ├── advanced/          # Advanced topics
│   ├── migration/         # Migration guides
│   ├── examples/          # Example walkthroughs
│   ├── api/               # API reference
│   ├── contributing/      # Contributing guides
│   └── resources/         # Additional resources
└── book/                  # Generated output (gitignored)
```

## Writing Documentation

### Markdown Format

mdBook uses GitHub-flavored Markdown with some extensions:

#### Code Blocks with Syntax Highlighting

\`\`\`rust
fn main() {
    println!("Hello, world!");
}
\`\`\`

#### Runnable Code Examples

\`\`\`rust,editable
# fn main() {
let x = 5;
println!("x = {}", x);
# }
\`\`\`

#### Including External Files

\`\`\`rust
{{#include ../../examples/basic.rs}}
\`\`\`

#### Links

- Internal: `[Getting Started](./getting-started/installation.md)`
- External: `[Rust Book](https://doc.rust-lang.org/book/)`
- Relative: `[API Index](../API_INDEX.md)`

#### Admonitions

> **Note**: Important information
>
> **Warning**: Critical warnings
>
> **Tip**: Helpful tips

### Adding New Pages

1. Create the markdown file in appropriate directory:
   ```bash
   touch src/examples/new-example.md
   ```

2. Add entry to `SUMMARY.md`:
   ```markdown
   - [New Example](./examples/new-example.md)
   ```

3. Write the content using the template below

### Page Template

```markdown
# Page Title

Brief introduction paragraph.

## Section 1

Content...

### Subsection

More content...

## Code Example

\`\`\`rust
// Example code
\`\`\`

## Next Steps

- [Related Topic 1](./related1.md)
- [Related Topic 2](./related2.md)
```

## Integrating with Rustdoc

### Generate API Documentation

```bash
cargo doc --no-deps --workspace
```

Output: `target/doc/`

### Link to Rustdoc

In markdown files, link to rustdoc:

```markdown
See [`ChatModel`](../../target/doc/dashflow_core/language_models/trait.ChatModel.html)
```

## Deployment

### GitHub Pages

1. Build the book:
   ```bash
   mdbook build
   ```

2. The output in `book/` can be deployed to GitHub Pages

### Continuous Integration

> **Note:** DashFlow does not ship with `.github/workflows/*` in this repository. The workflow below is a template for teams deploying the book from their own repo using GitHub Pages.

Add to `.github/workflows/docs.yml`:

```yaml
name: Documentation

on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2

      - name: Install mdBook
        run: |
          cargo install mdbook

      - name: Build documentation
        run: |
          cd docs/book
          mdbook build

      - name: Deploy to GitHub Pages
        uses: peaceiris/actions-gh-pages@v3
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./docs/book/book
```

## Style Guide

### Writing Style

- **Clear and Concise**: Use simple language
- **Active Voice**: "The function returns" not "The value is returned"
- **Present Tense**: "This does" not "This will do"
- **Code Examples**: Show, don't just tell
- **Consistent Terminology**: Use the same terms throughout

### Code Style

- Follow Rust conventions (rustfmt)
- Include error handling (`Result`, `?`)
- Add comments for complex logic
- Show complete, runnable examples
- Use meaningful variable names

### Example Structure

1. **What**: Brief description
2. **Why**: Use case or purpose
3. **How**: Code example
4. **Explanation**: Step-by-step breakdown
5. **Next Steps**: Related topics

## Maintenance

### Regular Updates

- Keep examples up-to-date with latest API
- Update version numbers in installation guides
- Review and fix broken links
- Add new features as they're implemented

### Link Checking

```bash
# Install mdbook-linkcheck
cargo install mdbook-linkcheck

# Check links
mdbook build
```

## Contributing

When contributing documentation:

1. Follow the style guide above
2. Test locally with `mdbook serve`
3. Check for broken links
4. Ensure code examples compile
5. Submit PR with documentation changes

## Resources

- [mdBook Documentation](https://rust-lang.github.io/mdBook/)
- [Markdown Guide](https://www.markdownguide.org/)
- [Rust Documentation Guidelines](https://doc.rust-lang.org/rustdoc/how-to-write-documentation.html)
