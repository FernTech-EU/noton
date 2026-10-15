# Noton

A lightweight notes app built with Teksilo, teksilo-scene and Qleany.
Implementation is in progress.

Build with crates.io releases pinned by Cargo.lock:

```sh
cargo build --locked
cargo run --locked
```

Choose **Start a note** or press Ctrl+N to begin typing immediately. An empty note
offers **Start writing**; double-click blank canvas space to add text there.
**Navigation** (Ctrl+Shift+\\) shows or hides the side panes. Search shows matching
excerpts and occurrence counts; activating a result selects its first matching
text, and clearing the query restores the previous note and view.

Drop PNG, JPEG or WebP images from a file manager onto a note to create separate
image frames. Drag their headers to move them and use the edge/corner handles to
resize. Delete removes the selected frame; Ctrl+Z restores it. Images are embedded
in the library. Drops carrying image bytes are also supported; URL-only browser
drags are not downloaded.

For dependency versions and test results,
see [Teksilo integration](docs/teksilo-integration.md).
See the [architecture dossier](docs/README.md) for the design and acceptance criteria.
