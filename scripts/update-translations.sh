#!/bin/bash

for dir in $(find po -type d)
do
  if [ -d "$dir/LC_MESSAGES" ]; then
    fname="$(basename "$dir")";
    (
      set -x
      msgmerge \
        --verbose \
        --no-fuzzy-matching \
        --backup=none \
        --update \
        "$dir/LC_MESSAGES/$fname.po" "po/boxbuddy.pot"
    )
  fi
done
