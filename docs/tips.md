# Tips

## Setting a Custom Home Directory with the Flatpak Version

The flatpak version of BoxBuddy is sandboxed, and cannot access the user's home directory unless granted permission. This can be done using the command line, or with an application called Flatseal

### Allowing Filesystem Access via the Command Line
You will need to determine if BoxBuddy is a user-level or system-level flatpak.

To do this, execute:

```bash
flatpak list --columns=app,installation | grep boxbuddyrs
```

This should say either "user" or "system".

If you have BoxBuddy as a user-level flatpak, execute:

```bash
flatpak override --user io.github.dvlv.boxbuddyrs --filesystem=home
```

If BoxBuddy is instead a system-level flatpak, execute:

```bash
sudo flatpak override io.github.dvlv.boxbuddyrs --filesystem=home
```

### Removing Filesystem Access
After creating your Box with a custom home directory, you may wish to remove filesystem permissions again.

If you have BoxBuddy as a user-level flatpak, execute:

```bash
flatpak override --user --reset io.github.dvlv.boxbuddyrs 
```

If BoxBuddy is instead a system-level flatpak, execute:

```bash
sudo flatpak override --reset io.github.dvlv.boxbuddyrs
```

------
