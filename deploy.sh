#!/usr/bin/env bash
# Deploy Origin of Life WASM to michelreij.nl via FTP (Plesk / hostingserver.nl)
#
# Usage:
#   FTP_USER=xxx FTP_PASS=xxx bash deploy.sh
# or:
#   bash deploy.sh <ftp_user> <ftp_pass>
#
# Requires lftp: brew install lftp

set -e
export PATH="/opt/homebrew/bin:$PATH"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PUBLIC_DIR="$SCRIPT_DIR/public"
FTP_HOST="mwp12009.hostingserver.nl"
FTP_PATH="/httpdocs/webapps/origin-of-life"

FTP_USER="${1:-${FTP_USER}}"
FTP_PASS="${2:-${FTP_PASS}}"

if [[ -z "$FTP_USER" || -z "$FTP_PASS" ]]; then
    echo "Usage: FTP_USER=xxx FTP_PASS=xxx bash deploy.sh"
    exit 1
fi

if [[ ! -f "$PUBLIC_DIR/main.js" ]]; then
    echo "Build output not found. Run 'npx webpack --mode production' first."
    exit 1
fi

echo "Deploying to ftp://$FTP_HOST$FTP_PATH ..."

# Upload top-level files
lftp -u "$FTP_USER","$FTP_PASS" "ftp://$FTP_HOST" <<LFTP
set ftp:ssl-allow yes
set ssl:verify-certificate no
cd $FTP_PATH
put -O . $PUBLIC_DIR/index.html
put -O . $PUBLIC_DIR/main.js
put -O . $PUBLIC_DIR/styles.css
put -O . $PUBLIC_DIR/styles.js
put -O . $PUBLIC_DIR/joy.js
put -O . $PUBLIC_DIR/particle-life-embed.js
put -O . $PUBLIC_DIR/particle-life-embed.html
LFTP

# Mirror shaders/
echo "Syncing shaders/ ..."
lftp -u "$FTP_USER","$FTP_PASS" "ftp://$FTP_HOST" <<LFTP
set ftp:ssl-allow yes
set ssl:verify-certificate no
mirror -R --delete --verbose --parallel=4 \
    $PUBLIC_DIR/shaders \
    $FTP_PATH/shaders
LFTP

# Mirror pkg/ (WASM + JS bindings, ~3.2 MB)
echo "Syncing pkg/ ..."
lftp -u "$FTP_USER","$FTP_PASS" "ftp://$FTP_HOST" <<LFTP
set ftp:ssl-allow yes
set ssl:verify-certificate no
mirror -R --delete --verbose --parallel=4 \
    $PUBLIC_DIR/pkg \
    $FTP_PATH/pkg
LFTP

# Sync newer source assets to public/ before uploading
echo "Checking assets/images/ for updates ..."
SRC_ASSETS="$SCRIPT_DIR/assets/images"
DST_ASSETS="$PUBLIC_DIR/assets/images"
mkdir -p "$DST_ASSETS"
for src_file in "$SRC_ASSETS"/*.png "$SRC_ASSETS"/*.jpg "$SRC_ASSETS"/*.gif "$SRC_ASSETS"/*.webp; do
    [[ -f "$src_file" ]] || continue
    dst_file="$DST_ASSETS/$(basename "$src_file")"
    if [[ ! -f "$dst_file" ]] || [[ "$src_file" -nt "$dst_file" ]]; then
        echo "  Copying $(basename "$src_file") -> public/assets/images/"
        cp "$src_file" "$dst_file"
    fi
done

# Mirror assets/images/ (copyright overlay PNGs)
echo "Syncing assets/images/ ..."
lftp -u "$FTP_USER","$FTP_PASS" "ftp://$FTP_HOST" <<LFTP
set ftp:ssl-allow yes
set ssl:verify-certificate no
mirror -R --delete --verbose --parallel=4 \
    $PUBLIC_DIR/assets/images \
    $FTP_PATH/assets/images
LFTP

echo ""
echo "Deploy complete."
echo "Direct URL : https://michelreij.nl/webapps/origin-of-life/index.html"
echo "WP embed   : paste public/particle-life-embed.html into a Custom HTML block"
