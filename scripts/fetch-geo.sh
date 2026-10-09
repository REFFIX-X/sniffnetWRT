#!/bin/sh
# Download GeoLite2 Country + ASN databases for geo/ASN enrichment.
# Requires a free MaxMind account + license key:
#   https://www.maxmind.com/en/geolite2/signup
#
# Usage:
#   MAXMIND_LICENSE_KEY=xxxx ./scripts/fetch-geo.sh [output-dir]
#   (default output-dir: ./db)

set -eu

KEY="${MAXMIND_LICENSE_KEY:?set MAXMIND_LICENSE_KEY (free GeoLite2 license key)}"
DIR="${1:-db}"

mkdir -p "$DIR"

for db in Country ASN; do
  url="https://download.maxmind.com/app/geoip_download?edition_id=GeoLite2-$db&license_key=$KEY&suffix=tar.gz"
  echo "Downloading GeoLite2-$db..."
  curl -fsSL "$url" -o "$DIR/GeoLite2-$db.tar.gz"
  tar -xzf "$DIR/GeoLite2-$db.tar.gz" -C "$DIR" --strip-components=1 --wildcards '*.mmdb'
  rm -f "$DIR/GeoLite2-$db.tar.gz"
done

echo "Done. Point the daemon at it with: --mmdb-dir $DIR"
