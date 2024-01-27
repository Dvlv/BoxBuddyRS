#!/bin/bash
XTR=$HOME/.cargo/bin/xtr

function update_pot(){
  echo "Generating new pot file...";
  $XTR src/main.rs -o po/boxbuddy.pot;
  echo "Done! New pot file created!";
}

function setup_dependencies(){
  echo "Installing xtr using cargo...";
  echo "Executing: cargo install xtr";
  cargo install xtr;
}

[ -f $XTR ] && echo "Dependencies: OK" || setup_dependencies

update_pot
