# Outliner

The lib browser: every open .LIB as a root, its entries grouped by type.

- Rows are 20px with zebra `gm-900`; hover `gm-700`; selected `amber-deep`; the active entry (the one Properties shows) adds `amber-bright` text and an `amber` icon.
- Group rows show the type badge and an entry count in `value-sm`.
- An `amber` dirty dot marks any entry, and its lib, that differs from disk.
- Header: filter field, type filter segmented buttons, and + to open another lib.
- Drag an entry from one lib onto another to copy it; drag onto an entry of the same type to open Graft with that pair.

The consumer provides the lib list, entry groups, selection, active entry, and dirty set.
