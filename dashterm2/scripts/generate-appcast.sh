#!/bin/bash
# DashTerm2 Appcast Generator
# Generates Sparkle-compatible appcast.xml from GitHub releases
#
# Usage: ./scripts/generate-appcast.sh
#
# Prerequisites:
# - gh CLI authenticated
# - Access to repository releases

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
APPCAST_PATH="$PROJECT_ROOT/appcast.xml"
REPO="dropbox/dTOOL/dashterm2"

echo "=== DashTerm2 Appcast Generator ==="
echo "Repository: $REPO"
echo ""

# Fetch releases from GitHub
echo "Fetching releases from GitHub..."
RELEASES=$(gh release list -R "$REPO" --limit 10 2>/dev/null || echo "")

if [[ -z "$RELEASES" ]]; then
    echo "WARNING: No releases found or gh CLI not configured"
    echo "Creating placeholder appcast.xml"
    cat > "$APPCAST_PATH" << 'APPCAST_EOF'
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <title>DashTerm2</title>
    <link>https://github.com/dropbox/dTOOL/dashterm2</link>
    <description>DashTerm2 Updates</description>
    <language>en</language>
    <!-- No releases available yet -->
  </channel>
</rss>
APPCAST_EOF
    echo "Created placeholder appcast.xml"
    exit 0
fi

# Start generating appcast
cat > "$APPCAST_PATH" << 'HEADER'
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle" xmlns:dc="http://purl.org/dc/elements/1.1/">
  <channel>
    <title>DashTerm2</title>
    <link>https://github.com/dropbox/dTOOL/dashterm2</link>
    <description>DashTerm2 Updates</description>
    <language>en</language>
HEADER

# Parse each release
echo "$RELEASES" | while read -r line; do
    # Format: TAG	TITLE	TYPE	PUBLISHED
    TAG=$(echo "$line" | awk -F'\t' '{print $1}')
    TITLE=$(echo "$line" | awk -F'\t' '{print $2}')
    TYPE=$(echo "$line" | awk -F'\t' '{print $3}')
    PUBLISHED=$(echo "$line" | awk -F'\t' '{print $4}')

    # Skip drafts
    if [[ "$TYPE" == "Draft" ]]; then
        continue
    fi

    # Get version (strip 'v' prefix if present)
    VERSION="${TAG#v}"

    # Get release info
    echo "Processing release: $TAG ($VERSION)"

    # Get download URL for DMG
    ASSETS=$(gh release view "$TAG" -R "$REPO" --json assets -q '.assets[].name' 2>/dev/null || echo "")
    DMG_NAME=$(echo "$ASSETS" | grep -E '\.dmg$' | head -1 || echo "")

    if [[ -z "$DMG_NAME" ]]; then
        echo "  WARNING: No DMG found for $TAG, skipping"
        continue
    fi

    DMG_URL="https://github.com/$REPO/releases/download/$TAG/$DMG_NAME"

    # Get file size (if we can download headers)
    SIZE=$(curl -sI "$DMG_URL" 2>/dev/null | grep -i content-length | awk '{print $2}' | tr -d '\r' || echo "0")

    # Get release notes
    NOTES=$(gh release view "$TAG" -R "$REPO" --json body -q '.body' 2>/dev/null | head -20 || echo "")

    # Check for signature file
    SIG_URL="https://github.com/$REPO/releases/download/$TAG/$DMG_NAME.signature"
    SIGNATURE=$(curl -sL "$SIG_URL" 2>/dev/null || echo "")

    # Format date for Sparkle (RFC 822)
    PUB_DATE=$(date -j -f "%Y-%m-%dT%H:%M:%SZ" "$PUBLISHED" "+%a, %d %b %Y %H:%M:%S %z" 2>/dev/null || echo "$PUBLISHED")

    # Write item
    cat >> "$APPCAST_PATH" << ITEM
    <item>
      <title>Version $VERSION</title>
      <sparkle:version>$VERSION</sparkle:version>
      <sparkle:shortVersionString>$VERSION</sparkle:shortVersionString>
      <pubDate>$PUB_DATE</pubDate>
      <description><![CDATA[$NOTES]]></description>
      <enclosure url="$DMG_URL"
                 length="$SIZE"
                 type="application/octet-stream"
ITEM

    if [[ -n "$SIGNATURE" ]]; then
        echo "                 sparkle:edSignature=\"$SIGNATURE\"" >> "$APPCAST_PATH"
    fi

    echo "      />" >> "$APPCAST_PATH"
    echo "    </item>" >> "$APPCAST_PATH"

    echo "  Added: $VERSION"
done

# Close appcast
cat >> "$APPCAST_PATH" << 'FOOTER'
  </channel>
</rss>
FOOTER

echo ""
echo "=== Appcast Generated ==="
echo "Output: $APPCAST_PATH"
echo ""
echo "To publish, commit and push appcast.xml to main branch."
echo "The URL https://raw.githubusercontent.com/$REPO/main/appcast.xml will serve it."
