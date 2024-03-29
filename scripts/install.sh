#/usr/bin/env bash
if [ -z "${XDG_DATA_HOME}" ]; then 
    DATAHOME=~/.local/share
else 
    DATAHOME=${XDG_DATA_HOME}
fi

if [ -z "${INSTALL_DIR}" ]; then 
    BIN_DIR=~/.local/bin
else 
    BIN_DIR=${INSTALL_DIR}/bin
fi


# copy binary and scripts
echo "Copying binary"
mkdir -p ${BIN_DIR}
cp boxbuddy-rs ${BIN_DIR}

# copy icons
echo "Copying icon"
mkdir -p ${DATAHOME}/icons/boxbuddy/
mkdir -p ${DATAHOME}/icons/hicolor/scalable/apps
cp -r *.svg ${DATAHOME}/icons/boxbuddy/
cp -r io.github.dvlv.boxbuddyrs.svg ${DATAHOME}/icons/hicolor/scalable/apps

# copy schemas
echo "Compiling Schema"
mkdir -p ${DATAHOME}/glib-2.0/schemas/
cp io.github.dvlv.boxbuddyrs.gschema.xml ${DATAHOME}/glib-2.0/schemas/
glib-compile-schemas ${DATAHOME}/glib-2.0/schemas/

# desktop
echo "Copying desktop file"
mkdir -p ${DATAHOME}/applications/
cp io.github.dvlv.boxbuddyrs.desktop ${DATAHOME}/applications/

# po
echo "Copying Translations";
mkdir -p ${DATAHOME}/locale;
cp -r po/* ${DATAHOME}/locale/

echo "BoxBuddy successfully installed!"
