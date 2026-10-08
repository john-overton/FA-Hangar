# GraftPanel

Merges chosen aspects of one lib entry into another, e.g. take ATF's F-14A geometry and hardpoints, keep USNF's flight envelope.

- Source and target slots on `gm-950` with `line-strong`; role in `section`, name in `title`-size label, lib in `value-sm`.
- One row per aspect: checkbox, icon, name, a mono diff summary (`ink-muted`), and a warning badge when the aspect conflicts.
- Conflicts are resolved before Apply: a `th-notice is-warn` lists them with the three choices (keep target, take source, offset).
- Preview in viewport draws the incoming geometry as a dashed `steel` ghost over the target.
- Apply graft is the panel's only primary button. Every graft is one undo step and marks the target dirty.

Aspects available depend on entry type: PT offers geometry (via its SH), LODs, hardpoints, envelope, propulsion, damage, textures; JT offers seeker, motor, and warhead blocks.
