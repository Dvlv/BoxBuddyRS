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


# remove binary and scripts
if [ -f ${BIN_DIR}/boxbuddy-rs ]; then
    echo "Removing binary";
    rm ${BIN_DIR}/boxbuddy-rs;
fi

# remove icons
if [ -d ${DATAHOME}/icons/boxbuddy ]; then
    echo "Removing icon";
    rm -r ${DATAHOME}/icons/boxbuddy;
fi

# desktop
if [ -f  ${DATAHOME}/applications/io.github.dvlv.boxbuddyrs.desktop ]; then
    echo "Removing desktop file"
    rm ${DATAHOME}/applications/io.github.dvlv.boxbuddyrs.desktop;
fi

echo "BoxBuddy successfully removed!"
