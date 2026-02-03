#!/usr/bin/env bash

set -ex

dir=$(dirname -- "$0")
USERAGENT='User-Agent: Reinze.com'

pull() {
  url="$1"
  filename="$2"

  curl -H "${USERAGENT}" "${url}" 2>/dev/null | jq | tee "${dir}/../lib/staging.json" && \
      cat "${dir}/../lib/staging.json" | jq && mv "${dir}/../lib/staging.json" "${dir}/../lib/${filename}.json";
}

pull "https://prices.runescape.wiki/api/v1/osrs/mapping" "item_db"
pull "https://prices.runescape.wiki/api/v1/osrs/latest" "ge"