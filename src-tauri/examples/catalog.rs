//! Prints the widget catalog as JSON.
//!
//! This exists so the catalog contract can be compared against the frontend's
//! `src/lib/catalog.ts` by `npm run check:catalog`. It is the machine-readable
//! form of `CATALOG_KINDS` and `default_config_for`, which together are the
//! backend half of that contract; keeping the emitter next to the definitions
//! means it cannot drift from them.

use hamon_lib::layout::{CatalogEntry, catalog_entries};

fn main() {
    let entries: Vec<CatalogEntry> = catalog_entries();
    match serde_json::to_string_pretty(&entries) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("failed to serialise the catalog: {error}");
            std::process::exit(1);
        }
    }
}
