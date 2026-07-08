# Drop deck files here

Any `.apkg` or `.csv` file placed directly in this folder is picked up
automatically by `make package` / `scripts/build_package.sh` — imported
on your PC at build time and shipped as `seed-data/` in the package, so
the device already has the deck(s) the first time it boots. No
on-device import step, no Parent Mode interaction needed.

```sh
cp ~/Downloads/Spanish101.apkg deploy/knulli/decks/
make package
```

Multiple files accumulate into one database (imports are additive —
see PACKAGING.md). To use specific files without touching this folder,
pass `--deck` explicitly instead (this folder is then ignored):

```sh
scripts/build_package.sh --deck ~/Decks/A.apkg --deck ~/Decks/B.apkg
# or: make package DECKS="~/Decks/A.apkg ~/Decks/B.apkg"
```

Files here are gitignored — they're personal content, not part of the
app. Export from Anki desktop with **"Support older Anki versions"**
checked (see PACKAGING.md for why that matters).
