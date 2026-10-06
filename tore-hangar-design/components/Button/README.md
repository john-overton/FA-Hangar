# Button

Buttons are flat gunmetal blocks with a 1px `gm-1000` keyline and a 1px `lip` highlight; the only colored button is the one action a dialog exists for.

- **Default** (`th-btn`): `gm-700` fill, `ink` label, hover `gm-600`, pressed `gm-950` with no lip.
- **Primary** (`th-btn-primary`): `amber` fill, `on-amber` label. At most one per dialog or panel: Apply graft, Save lib, Package.
- **Toggle on** (`is-on`): `amber-deep` fill, `amber` label or icon. Use for viewport overlays, shading modes, and dock tabs.
- **Icon** (`th-btn-icon`): 22 × 22, 16px icon. Always give it a `title`; the status bar shows the title on hover.
- **Segmented** (`th-seg`): joined buttons for mutually exclusive modes. Two to five members.
- **Danger** (`th-btn-danger`): `danger` label on the default fill, always with the close or warning icon.
- **Ghost** (`th-btn-ghost`): editor header menus (View, Select, Mesh) and Cancel.

Height is fixed at 22px. Labels are verbs in sentence case: "Apply graft", not "OK". The consumer provides the label, optional leading icon, and `title`.
