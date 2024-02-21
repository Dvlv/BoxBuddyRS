application_id = 'io.github.dvlv.boxbuddyrs'

scalable_dir = 'hicolor' / 'scalable' / 'apps'
install_data(
  scalable_dir / ('@0@.svg').format(application_id),
  install_dir: get_option('datadir') / 'icons' / scalable_dir
)

symbolic_apps_dir = 'hicolor' / 'symbolic' / 'apps'
install_data(
  symbolic_apps_dir / ('@0@-symbolic.svg').format(application_id),
  install_dir: get_option('datadir') / 'icons' / symbolic_apps_dir
)

symbolic_dir = 'hicolor' / 'symbolic'
install_data(
  symbolic_dir / ('build-alt-symbolic.svg').format(application_id),
  install_dir: get_option('datadir') / 'icons' / scalable_dir
)
install_data(
  symbolic_dir / ('build-alt-symbolic-light.svg').format(application_id),
  install_dir: get_option('datadir') / 'icons' / scalable_dir
)

