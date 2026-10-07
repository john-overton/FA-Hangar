//! Explicit state-switch inspection and bounded placement edits on source parts.
use super::view::{label_fit, text_fit, Action, Layout};
use super::*;
use hangar_core::animation;
impl App {
    pub(super) fn open_animation(&mut self) {
        self.finish_stroke();
        self.mesh_edit = false;
        self.mesh_drag = None;
        self.hp_tool = false;
        self.hp_visible = false;
        self.hp_drag = None;
        self.model_paint = false;
        self.paint_enabled = false;
        self.decal_active = false;
        self.decal_draft = None;
        self.selected_face = None;
        self.mode = Mode::Model;
        self.media_tab = 0;
        self.animation_tool = true;
        let result = self.animation_preview();
        self.result(result);
    }
    fn animation_bytes(&self) -> Result<Vec<u8>> {
        let i = self
            .model_entry
            .ok_or("Open the shape owner's LIB for animation tools")?;
        self.doc.archive.entries[i].read()
    }
    pub(super) fn animation_preview(&mut self) -> Result<()> {
        let bytes = self.animation_bytes()?;
        let preview = Model::with_state(&bytes, &self.animation_state)?;
        for name in &preview.textures {
            let key = name.clone();
            let name = if name.contains('.') {
                name.clone()
            } else {
                format!("{name}.PIC")
            };
            if let Some((_, _, entry)) = self.resolve_resource(&name) {
                if let Ok(bytes) = entry.read() {
                    if let Ok(pic) = Pic::parse(&bytes) {
                        self.textures.insert(key, pic);
                    }
                }
            }
        }
        self.animation_part = self
            .animation_part
            .min(preview.parts.len().saturating_sub(1));
        self.preview = Some(preview);
        self.status = "State-switch preview / native angle arithmetic is not executed".into();
        Ok(())
    }
    pub(super) fn set_animation_state(&mut self, address: usize, text: &str) -> Result<()> {
        let value: i32 = text
            .parse()
            .map_err(|_| "Enter a signed word state value")?;
        if !(-32768..=32767).contains(&value) {
            return Err("State exceeds signed word range".into());
        }
        if self.animation_state.is_empty() {
            self.animation_symbols = self
                .animation_bytes()
                .and_then(|b| animation::symbols(&b))
                .unwrap_or_default();
        }
        let old = self.animation_state.insert(address, value);
        if let Err(error) = self.animation_preview() {
            if let Some(old) = old {
                self.animation_state.insert(address, old);
            } else {
                self.animation_state.remove(&address);
            }
            return Err(error);
        }
        Ok(())
    }
    /// State keys are absolute guard addresses. Keep them only while the shape's import
    /// symbols (and so its trampoline aliases) are unchanged; any undo, redo or replace
    /// that moves them clears the preview states. Returns true when states were reset.
    pub(super) fn revalidate_animation_state(&mut self) -> bool {
        if self.animation_state.is_empty() {
            return false;
        }
        let now = self
            .animation_bytes()
            .and_then(|b| animation::symbols(&b))
            .ok();
        if now.as_ref() == Some(&self.animation_symbols) {
            return false;
        }
        self.animation_state.clear();
        self.animation_scroll = 0;
        true
    }
    pub(super) fn animation_addresses(&self) -> alloc::collections::BTreeSet<usize> {
        let mut addresses = self
            .preview
            .as_ref()
            .or(self.model.as_ref())
            .map(|m| m.state_words.clone())
            .unwrap_or_default();
        if let Some(m) = &self.model {
            addresses.extend(&m.state_words);
        }
        addresses.extend(self.animation_state.keys());
        addresses
    }
    pub(super) fn animation_rows(&self) -> usize {
        ((self.height - 390) / 28).max(1) as usize
    }
    pub(super) fn part_position_prompt(&mut self, axis: usize) {
        let Some(part) = self
            .preview
            .as_ref()
            .or(self.model.as_ref())
            .and_then(|m| m.parts.get(self.animation_part))
        else {
            return;
        };
        self.prompt = Some(Prompt {
            kind: PromptKind::PartPosition(axis),
            title: format!(
                "Part {} placement / {} source coordinate",
                self.animation_part + 1,
                ["X", "Y", "Z"][axis]
            ),
            value: part.position[axis].to_string(),
            axis,
        });
    }
    pub(super) fn part_position(&mut self, axis: usize, text: &str) -> Result<()> {
        let i = self.model_entry.ok_or("No local shape")?;
        let offset = self
            .preview
            .as_ref()
            .or(self.model.as_ref())
            .and_then(|m| m.parts.get(self.animation_part))
            .ok_or("No part")?
            .offset;
        let value = text
            .parse()
            .map_err(|_| "Enter a signed source coordinate")?;
        let bytes = animation::place_part(
            &self.animation_bytes()?,
            &self.animation_state,
            offset,
            axis,
            value,
        )?;
        self.doc.replace(i, bytes)?;
        self.refresh();
        self.animation_preview()?;
        self.status = "Part placement updated / source animation retained / one undo".into();
        Ok(())
    }
    pub(super) fn animation_inspector(&self, o: &mut Layout) {
        let r = self.right();
        let w = self.width - r;
        o.canvas
            .label(r + 12, 44, "ANIMATION / source parts", c::INK);
        let Some(model) = self.preview.as_ref().or(self.model.as_ref()) else {
            return;
        };
        label_fit(
            &mut o.canvas,
            r + 12,
            72,
            w - 24,
            "State switches / placement only",
            c::AMBER,
        );
        label_fit(
            &mut o.canvas,
            r + 12,
            92,
            w - 24,
            "Native rotation code is retained.",
            c::INK_MUTED,
        );
        o.button(
            [r + 12, 105, w - 24, 24],
            "Reset preview states",
            Action::AnimationReset,
            false,
        );
        o.button([r + 12, 139, 32, 24], "<", Action::AnimationPart(-1), false);
        text_fit(
            &mut o.canvas,
            r + 55,
            155,
            w - 110,
            &format!(
                "Part {} / {}",
                if model.parts.is_empty() {
                    0
                } else {
                    self.animation_part + 1
                },
                model.parts.len()
            ),
            c::STEEL,
        );
        o.button(
            [self.width - 44, 139, 32, 24],
            ">",
            Action::AnimationPart(1),
            false,
        );
        if let Some(part) = model.parts.get(self.animation_part) {
            for (k, label) in ["X", "Y", "Z"].iter().enumerate() {
                o.button(
                    [r + 12, 173 + k as i32 * 28, w - 24, 24],
                    &format!("{label}  {}", part.position[k]),
                    Action::PartPosition(k),
                    false,
                );
            }
            text_fit(
                &mut o.canvas,
                r + 12,
                273,
                w - 24,
                &format!("C4 @ {:X} -> {:X}", part.offset, part.target),
                c::INK_MUTED,
            );
            text_fit(
                &mut o.canvas,
                r + 12,
                293,
                w - 24,
                &format!("Stored angles {:?}", part.rotation),
                c::INK_MUTED,
            );
        } else {
            label_fit(
                &mut o.canvas,
                r + 12,
                192,
                w - 24,
                "No C4 parts in this state.",
                c::INK_MUTED,
            );
        }
        let names = self
            .animation_bytes()
            .and_then(|b| animation::symbols(&b))
            .unwrap_or_default();
        let addresses = self.animation_addresses();
        let rows = self.animation_rows();
        let first = self
            .animation_scroll
            .min(addresses.len().saturating_sub(rows));
        label_fit(
            &mut o.canvas,
            r + 12,
            321,
            w - 24,
            &if addresses.len() > rows {
                format!(
                    "State inputs {}-{} of {} / wheel scrolls",
                    first + 1,
                    (first + rows).min(addresses.len()),
                    addresses.len()
                )
            } else {
                "Imported state inputs / preview only".into()
            },
            c::INK,
        );
        for (row, address) in addresses.iter().skip(first).take(rows).enumerate() {
            let label = names
                .get(address)
                .cloned()
                .unwrap_or_else(|| format!("{address:08X}"));
            o.button(
                [r + 12, 332 + row as i32 * 28, w - 24, 24],
                &format!(
                    "{label} = {}",
                    self.animation_state.get(address).copied().unwrap_or(0)
                ),
                Action::AnimationState(*address),
                self.animation_state.get(address).copied().unwrap_or(0) != 0,
            );
        }
        if addresses.is_empty() {
            label_fit(
                &mut o.canvas,
                r + 12,
                354,
                w - 24,
                "No reviewed state guards reached.",
                c::INK_MUTED,
            );
        }
        label_fit(
            &mut o.canvas,
            r + 12,
            self.height - 48,
            w - 24,
            "Not a full animation interpreter.",
            c::INK_MUTED,
        );
    }
}

#[inline(never)]
fn boxed_app() -> Box<App> {
    Box::new(App::new())
}
impl App {
    #[inline(never)]
    pub(super) fn smoke_advanced_tools(&mut self) {
        let mut a = boxed_app();
        a.demo();
        let nt = hangar_core::brf::demo_npc();
        a.doc
            .transaction(vec![Entry::new("DEMO.NT", nt).unwrap()], &[])
            .unwrap();
        a.select_entry(a.doc.archive.find("DEMO.NT").unwrap());
        assert_eq!(a.hp_context.as_ref().unwrap().stations.len(), 2);
        a.act(Action::Hardpoints);
        let find = |a: &App, predicate: &dyn Fn(Action) -> bool| {
            a.chrome_hit(predicate).expect("Station control missing")
        };
        let press = |a: &mut App, r: [i32; 4], dx: i32| a.smoke_drag(r, dx);
        let slew = find(&a, &|x| matches!(x, Action::StationSlew(true)));
        press(&mut a, slew, 0);
        assert!(a.hp_slew);
        // The heading limit is a NumberField: a click types the exact value.
        use super::widgets::NumberTarget;
        let limit = find(&a, &|x| {
            matches!(x, Action::Number(NumberTarget::Station(6)))
        });
        press(&mut a, limit, 0);
        assert!(a.prompt.is_some());
        a.key(Key::Char('a'), true, false);
        for ch in "15000".chars() {
            a.key(Key::Char(ch), false, false);
        }
        a.key(Key::Enter, false, false);
        assert!(a.prompt.is_none(), "{}", a.status);
        assert_eq!(
            a.hp_context
                .as_ref()
                .unwrap()
                .brf
                .fields
                .iter()
                .find(|f| f.label == "hardpoint[0].slewLimitH")
                .unwrap()
                .value,
            "15000"
        );
        a.act(Action::Undo);
        // The X location is an axis-coloured vector NumberField: scrub, then
        // Backspace returns the operand on disk; undo restores each step.
        a.doc.mark_saved();
        a.refresh();
        let before = a.doc.archive.bytes().unwrap();
        let t = NumberTarget::Station(1);
        let x0 = a.number_spec(t).unwrap().value;
        let field = find(&a, &|x| matches!(x, Action::Number(n) if n == t));
        assert!(a.layout().canvas.commands.iter().any(|d| matches!(d,
            Draw::Text(_, _, s, color, _) if s == "X" && *color == c::AXIS_X.0)));
        press(&mut a, field, 12);
        assert_eq!(a.number_spec(t).unwrap().value, x0 + 6);
        assert_eq!(
            a.hp_context.as_ref().unwrap().stations[0].position[0] as i64,
            x0 + 6
        );
        a.motion(field[0] + field[2] / 2, field[1] + field[3] / 2, false);
        a.key(Key::Backspace, false, false);
        assert_eq!(a.number_spec(t).unwrap().value, x0);
        assert_eq!(a.doc.archive.bytes().unwrap(), before);
        a.act(Action::Undo);
        assert_eq!(a.number_spec(t).unwrap().value, x0 + 6);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), before);
        let next = find(&a, &|x| matches!(x, Action::HardpointStep(1)));
        press(&mut a, next, 0);
        assert_eq!(a.hp_selected, 1);
        let mut shape = model::demo_shape();
        let mut code = vec![0xc4, 0, 10, 0, 30, 0, 20, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0];
        code.extend(&shape[256..]);
        shape.truncate(256);
        shape.extend(&code);
        for at in [128, 136] {
            shape[at..at + 4].copy_from_slice(&(code.len() as u32).to_le_bytes());
        }
        a.doc.replace(0, shape).unwrap();
        a.doc.mark_saved();
        a.select_entry(0);
        let original = a.doc.archive.bytes().unwrap();
        a.open_animation();
        assert!(a.animation_tool);
        assert!(!a.doc.dirty());
        assert_eq!(a.preview.as_ref().unwrap().parts[0].position, [10, 20, 30]);
        // Byte-local placement edits keep import addresses, so preview states survive.
        a.set_animation_state(0x7912, "1").unwrap();
        a.part_position(1, "40").unwrap();
        assert_eq!(a.preview.as_ref().unwrap().parts[0].position, [10, 40, 30]);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        assert_eq!(a.preview.as_ref().unwrap().parts[0].position, [10, 20, 30]);
        assert_eq!(a.animation_state.get(&0x7912), Some(&1));
        // A history step whose import aliases differ invalidates absolute state keys.
        a.animation_symbols.insert(0x7912, "_PLgearDown".into());
        a.act(Action::Redo);
        assert!(a.animation_state.is_empty());
        a.act(Action::Undo);
        // State inputs beyond the visible rows scroll instead of being hidden.
        for n in 0..40 {
            a.animation_state.insert(0x9000 + n * 2, 0);
        }
        a.animation_symbols = animation::symbols(&a.animation_bytes().unwrap()).unwrap();
        a.mouse = [a.right() + 20, a.height - 80];
        let rows = a.animation_rows();
        a.wheel(-100);
        assert_eq!(a.animation_scroll, a.animation_addresses().len() - rows);
        assert!(a
            .layout()
            .hits
            .iter()
            .any(|h| matches!(h.action, Action::AnimationState(at) if at == 0x9000 + 39 * 2)));
        a.wheel(100);
        assert_eq!(a.animation_scroll, 0);
        a.animation_state.clear();
        for (w, h) in [(800, 600), (1280, 800)] {
            a.width = w;
            a.height = h;
            for hit in a.layout().hits {
                assert!(
                    hit.rect[0] >= 0
                        && hit.rect[1] >= 0
                        && hit.rect[0] + hit.rect[2] <= w
                        && hit.rect[1] + hit.rect[3] <= h,
                    "Advanced tool bounds"
                );
            }
        }
        a.part_position(0, "12").unwrap();
        a.close_library().unwrap();
        assert!(a.prompt.is_some());
        a.act(Action::Cancel);
        assert!(a.doc.dirty());
        a.close_library().unwrap();
        let hit = a
            .layout()
            .hits
            .into_iter()
            .find(|h| matches!(h.action, Action::DiscardChanges))
            .unwrap();
        a.click(hit.rect[0] + 5, hit.rect[1] + 5, 1, true);
        assert!(a.doc.archive.entries.is_empty());
        assert!(!a.quit);
        a.demo();
        a.doc.replace(0, model::demo_shape()).unwrap();
        a.close();
        a.act(Action::Cancel);
        assert!(!a.quit);
        a.close();
        a.act(Action::DiscardChanges);
        assert!(a.quit);
    }
}
