#!/bin/bash
# DashTerm2 Release Script
# Creates a signed release and updates the appcast.xml
#
# Usage: ./scripts/create-release.sh <version>
# Example: ./scripts/create-release.sh 3.6.0
#
# Prerequisites:
# - EdDSA key generated and stored in Keychain (run: submodules/Sparkle/bin/generate_keys)
# - GitHub CLI (gh) installed and authenticated
# - Xcode build completed successfully

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

usage() {
    echo "Usage: $0 <version>"
    echo "Example: $0 3.6.0"
    exit 1
}

log() {
    echo -e "${GREEN}[RELEASE]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

# Check arguments
if [[ $# -ne 1 ]]; then
    usage
fi

VERSION="$1"

# Validate version format (semver-like)
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9]+)?$ ]]; then
    error "Invalid version format: $VERSION (expected: X.Y.Z or X.Y.Z-suffix)"
fi

# Check prerequisites
log "Checking prerequisites..."

if ! command -v gh &> /dev/null; then
    error "GitHub CLI (gh) not found. Install with: brew install gh"
fi

if ! gh auth status &> /dev/null; then
    error "GitHub CLI not authenticated. Run: gh auth login"
fi

SIGN_UPDATE="$PROJECT_ROOT/submodules/Sparkle/bin/sign_update"
if [[ ! -x "$SIGN_UPDATE" ]]; then
    error "sign_update tool not found. Build it with:"
    echo "  cd submodules/Sparkle && xcodebuild -scheme 'sign_update' -configuration Release -derivedDataPath /tmp/sparkle-build build CODE_SIGNING_ALLOWED=NO CODE_SIGN_IDENTITY='-'"
    echo "  cp /tmp/sparkle-build/Build/Products/Release/sign_update submodules/Sparkle/bin/"
    exit 1
fi

# Find the built app
APP_PATH="$(find ~/Library/Developer/Xcode/DerivedData/DashTerm2-*/Build/Products/Development -name "DashTerm2.app" -print -quit 2>/dev/null || echo "")"
if [[ -z "$APP_PATH" ]] || [[ ! -d "$APP_PATH" ]]; then
    error "DashTerm2.app not found. Build with:"
    echo "  xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 -configuration Development build CODE_SIGNING_ALLOWED=NO CODE_SIGN_IDENTITY='-'"
    exit 1
fi

log "Found app: $APP_PATH"

# Create release directory
RELEASE_DIR="/tmp/dashterm2-release-$VERSION"
rm -rf "$RELEASE_DIR"
mkdir -p "$RELEASE_DIR"

log "Creating release in: $RELEASE_DIR"

# Copy app to release directory
cp -R "$APP_PATH" "$RELEASE_DIR/DashTerm2.app"

# Create ZIP archive
ZIP_NAME="DashTerm2-$VERSION.zip"
ZIP_PATH="$RELEASE_DIR/$ZIP_NAME"

log "Creating ZIP archive: $ZIP_NAME"
cd "$RELEASE_DIR"
ditto -c -k --sequesterRsrc --keepParent "DashTerm2.app" "$ZIP_NAME"

# Get file size
FILE_SIZE=$(stat -f%z "$ZIP_PATH")
log "Archive size: $FILE_SIZE bytes"

# Sign the archive
log "Signing archive with EdDSA..."
SIGNATURE=$("$SIGN_UPDATE" --sign-update "$ZIP_PATH" 2>&1 | grep -o 'sparkle:edSignature="[^"]*"' | cut -d'"' -f2 || true)

if [[ -z "$SIGNATURE" ]]; then
    # Try alternative output format
    SIGNATURE=$("$SIGN_UPDATE" "$ZIP_PATH" 2>&1)
fi

if [[ -z "$SIGNATURE" ]]; then
    error "Failed to sign archive. Make sure EdDSA key is in Keychain."
fi

log "Signature: ${SIGNATURE:0:40}..."

# Generate pubDate
PUB_DATE=$(date -R)

# Create appcast entry
APPCAST_ENTRY="        <item>
            <title>Version $VERSION</title>
            <pubDate>$PUB_DATE</pubDate>
            <sparkle:version>$VERSION</sparkle:version>
            <sparkle:shortVersionString>$VERSION</sparkle:shortVersionString>
            <sparkle:minimumSystemVersion>10.15</sparkle:minimumSystemVersion>
            <enclosure
                url=\"https://github.com/dropbox/dTOOL/dashterm2/releases/download/v$VERSION/$ZIP_NAME\"
                sparkle:edSignature=\"$SIGNATURE\"
                length=\"$FILE_SIZE\"
                type=\"application/octet-stream\"/>
            <description><![CDATA[
                <h2>What's New in $VERSION</h2>
                <ul>
                    <li>See release notes on GitHub</li>
                </ul>
            ]]></description>
        </item>"

log "Generated appcast entry"

# Print summary
echo ""
echo "============================================"
echo "Release $VERSION prepared successfully!"
echo "============================================"
echo ""
echo "ZIP archive: $ZIP_PATH"
echo "Size: $FILE_SIZE bytes"
echo "Signature: ${SIGNATURE:0:60}..."
echo ""
echo "Next steps:"
echo ""
echo "1. Create GitHub release:"
echo "   gh release create v$VERSION \"$ZIP_PATH\" --title \"DashTerm2 $VERSION\" --notes \"Release notes here\""
echo ""
echo "2. Update appcast.xml by adding this entry after the <language> tag:"
echo ""
echo "$APPCAST_ENTRY"
echo ""
echo "3. Commit and push appcast.xml:"
echo "   git add appcast.xml && git commit -m \"Release v$VERSION\" && git push"
echo ""
echo "============================================"

# Optionally, ask to proceed automatically
read -p "Would you like to create the GitHub release now? [y/N] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    log "Creating GitHub release..."

    # Create release with ZIP
    gh release create "v$VERSION" "$ZIP_PATH" \
        --title "DashTerm2 $VERSION" \
        --notes "## DashTerm2 $VERSION

Download DashTerm2-$VERSION.zip and extract the app.

### Installation
1. Download and extract the ZIP file
2. Move DashTerm2.app to /Applications
3. Right-click and select \"Open\" (required first time for unsigned apps)

### Changes
See commit history for details."

    log "GitHub release created!"

    # Update appcast.xml
    log "Updating appcast.xml..."

    # Insert entry after <language>en</language>
    cd "$PROJECT_ROOT"

    # Create updated appcast
    python3 - "$APPCAST_ENTRY" << 'PYTHON_SCRIPT'
import sys
import re

entry = sys.argv[1]
appcast_path = 'appcast.xml'

with open(appcast_path, 'r') as f:
    content = f.read()

# Find the position after <language>en</language> and any comments
# Insert new entry before closing </channel>
pattern = r'(</channel>)'
replacement = entry + '\n    \\1'

if '<item>' in content:
    # If there are existing items, insert before the first one
    pattern = r'(<language>en</language>\s*(?:<!--.*?-->)?)'
    replacement = '\\1\n' + entry
    new_content = re.sub(pattern, replacement, content, flags=re.DOTALL)
else:
    # No existing items, insert before </channel>
    new_content = content.replace('</channel>', entry + '\n    </channel>')

with open(appcast_path, 'w') as f:
    f.write(new_content)

print('appcast.xml updated')
PYTHON_SCRIPT

    log "appcast.xml updated!"
    echo ""
    echo "Don't forget to commit and push:"
    echo "  git add appcast.xml && git commit -m \"Update appcast for v$VERSION\" && git push"
fi

log "Done!"
