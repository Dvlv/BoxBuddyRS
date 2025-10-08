# Translations

## To add a new translation

- Make a folder here named after the locale, e.g. `mkdir fr_FR`
- Make a folder inside there called `LC_MESSAGES`, e.g. `mkdir fr_FR/LC_MESSAGES`
- Copy `boxbuddy.pot` to your-locale.po inside that folder, e.g. `cp boxbuddy.pot fr_FR/LC_MESSAGES/fr_FR.po`
- Fill in the translations in your new `.po` file. This can be done using [https://poedit.net](https://poedit.net/)
- (Optional) compile your `.po` to an `.mo` by running `make translate`.
    - I don't mind doing this step, so please feel free to commit a PR with just the new `.po` file
- Make a Pull Request as normal.

## Testing a translation

- Open up `src/utils.rs` and find the comment which starts with `--TRANSLATORS:`
- Comment out the if/else statement below by adding `//` to the start of the lines
- Run `cargo run` as normal

## Updating a `po` file from a new `pot` file

To avoid having to manually rebase your po files each time the pot file changes (for example when all the line numbers change), you can simply run `make update-translations`.
