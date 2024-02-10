# Adding Translations
To translate BoxBuddy, please follow these instructions: 

- Make a folder inside BoxBuddy's `po` folder named after the locale, e.g. `mkdir po/fr_FR`
- Make a folder inside there called `LC_MESSAGES`, e.g. `mkdir po/fr_FR/LC_MESSAGES`
- Copy the `boxbuddy.pot` file to to your-locale.po inside that folder, e.g. `cp po/boxbuddy.pot po/fr_FR/LC_MESSAGES/fr_FR.po`
- Fill in the translations in your new `.po` file. This can be done using [https://poedit.net](https://poedit.net/)
- (Optional) compile your `.po` to an `.mo` by running `make translate`.
    - I don't mind doing this step, so please feel free to commit a PR with just the new `.po` file
- Make a Pull Request as normal.

## Testing a translation

- Open up `src/utils.rs` and find the comment which starts with `--TRANSLATORS:`
- Comment out the if/else statement below by adding `//` to the start of the lines
- Run `cargo run` as normal
