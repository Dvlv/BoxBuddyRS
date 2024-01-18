#!/bin/bash
XTR=$HOME/.cargo/bin/xtr

function update_pod(){
  echo "Generating new pod file..."
  $HOME/.cargo/bin/xtr src/main.rs -o po/boxbuddy.pot
  echo "Done. New pot file created, please translate."
}

function setup_dependencies(){
  echo "Install xtr using cargo..."
  cargo install xtr
}

[ -f $XTR ] && echo "Dependencies: OK" || setup_dependencies

update_pod
