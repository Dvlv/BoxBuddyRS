#!/bin/bash
APPDATA_VER=$(grep '<release version="' io.github.dvlv.boxbuddyrs.metainfo.xml -m 1 | sed -n '/<release version="/s/<release version="//p' | sed 's/".*//' | sed 's/^[[:space:]]*//');
RUST_VER=$(grep "set_version" src/main.rs | sed -n '/d.set_version("/s/d.set_version("//p' | sed 's/.\{3\}$//' | sed 's/^[[:space:]]*//');
DIFF=$(if [ "$RUST_VER" = "$APPDATA_VER" ]; then echo 0; else echo 1; fi)

echo $APPDATA_VER;

exit $DIFF;