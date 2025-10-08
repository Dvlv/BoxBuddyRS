# BoxBuddy
An unofficial GUI for managing your Distroboxes. Written with GTK4 + Libadwaita.

Note that this does not come with Podman or Distrobox, those will need to be installed on the host.

![Main Menu](docs/screenshot-1.png)

## Installing

### Flatpak
<a href='https://flathub.org/apps/io.github.dvlv.boxbuddyrs'>
  <img width='240' alt='Download on Flathub' src='https://dl.flathub.org/assets/badges/flathub-badge-en.png'/>
</a>

**Note**: The Flatpak does not have filesystem access by default, so some features are disabled. Please see [the documentation for details.](https://dvlv.github.io/BoxBuddyRS/tips)

### Local Binary
- Visit the [releases section](https://github.com/Dvlv/BoxBuddyRS/releases)
- Download and unzip the release
- Execute `./install.sh`

## Documentation
Documentation lives at the [GitHub Pages site.](https://dvlv.github.io/BoxBuddyRS)

## Contributing
Please see the dedicated [Dev Documentation](https://dvlv.github.io/BoxBuddyRS/dev-docs) and [Translator Documentation.](https://dvlv.github.io/BoxBuddyRS/translator-docs)

## Help Wanted
Help would be greatly appreciated in these areas:

- **Translations**
  - Currently implemented:
    - Czech (cs)
    - German (de_DE)
    - Greek (el)
    - Spanish (es)
    - French (fr_FR)
    - Hindi (hi)
    - Italian (it_IT)
    - Netherlands (nl_NL)
    - Polish (pl_PL)
    - Portuguese (pt_BR)
    - Russian (ru_RU)
    - Ukranian (uk_UA)
    - Chinese (zh_CN, zh_TW)
  - Any other translations appreciated!

## Issues / Feature Requests

Feature requests are welcome! Please check the [roadmap](https://github.com/Dvlv/BoxBuddyRS/blob/master/docs/ROADMAP.md) to see if a feature is already planned.

When filing issues, please keep in mind that BoxBuddy is just a GUI, and I am not a developer of Distrobox itself. Any issues with created boxes are probably better logged [upstream.](https://github.com/89luca89/distrobox/issues)
