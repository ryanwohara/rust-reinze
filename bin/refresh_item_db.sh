#!/usr/bin/env bash

dir=$(dirname -- "$0")

curl https://prices.runescape.wiki/api/v1/osrs/mapping 2>/dev/null | jq > ${dir}/../lib/item_db.json