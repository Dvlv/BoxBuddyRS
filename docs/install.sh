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
cp -r io.github.dvlv.boxbuddyrs.svg ${DATAHOME}/icons/boxbuddy/
cp -r io.github.dvlv.boxbuddyrs.svg ${DATAHOME}/icons/hicolor/scalable/apps

# desktop
echo "Copying desktop file"
mkdir -p ${DATAHOME}/applications/
cp io.github.dvlv.boxbuddyrs.desktop ${DATAHOME}/applications/

echo "BoxBuddy successfully installed!"
