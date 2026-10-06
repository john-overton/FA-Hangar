# Third-party notices

## PKWare DCL decoding

The Rust raw-literal DCL decoder in `crates/hangar-core/src/dcl.rs` uses the
format tables, canonical decoding method, and public test vector documented by
Mark Adler's [blast](https://github.com/madler/zlib/tree/master/contrib/blast),
also consulted through the local USNF-ATF reference decoder. This is an altered,
bounded Rust implementation, not the original blast distribution. It supports
raw literals only, limits output, and returns allocation-backed text errors.

The upstream notice from `blast.h` follows:

```text
Copyright (C) 2003, 2012, 2013 Mark Adler

This software is provided 'as-is', without any express or implied
warranty.  In no event will the author be held liable for any damages
arising from the use of this software.

Permission is granted to anyone to use this software for any purpose,
including commercial applications, and to alter it and redistribute it
freely, subject to the following restrictions:

1. The origin of this software must not be misrepresented; you must not
   claim that you wrote the original software. If you use this software
   in a product, an acknowledgment in the product documentation would be
   appreciated but is not required.
2. Altered source versions must be plainly marked as such, and must not be
   misrepresented as being the original software.
3. This notice may not be removed or altered from any source distribution.

Mark Adler    madler@alumni.caltech.edu
```

## T.O.R.E-Fighters

The DCL decoder, bounded PIC/PCM readers and BRF field-order tables are adapted from the GPL-3.0
T.O.R.E-Fighters project. The bounded shape reader and archive writer were
implemented with its readers, exporter, and format notes as references.
Source: https://github.com/john-overton/T.O.R.E-Fighters

All modifications in Hangar are separate from the upstream implementation.
No retail game content is included. Users supply their own LIB files.
The supplied design system uses system font fallbacks; no external fonts ship.
