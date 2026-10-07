//! Forward "may" dataflow over the SH control-flow graph, for the texture
//! state and vertex-slot proofs in `shape_geometry`.
//!
//! The graph is built from one `Step` per inventory record. A node is a
//! record together with the 38 scope end in force when it is reached (as
//! `Model` tracks it per call frame): a 1E is padding while a scope covers
//! it and a return otherwise, and 00 always returns. Calls are not inlined:
//! each called block is a procedure, analysed once from a symbolic entry
//! state, and a call applies the callee's summary (the set of states its
//! returns leave, with `ENTRY` standing for "whatever the caller had"). This
//! is exact over valid call/return paths for a last-writer analysis, so
//! unrelated call sites never merge. Loops need no special case: values only
//! grow, are joined where paths meet, and the worklist stops at the fixed
//! point.
//!
//! Every value is a small set: `ENTRY`, `UNDEFINED` (nothing written since
//! the shape start), a writer record, or "unknown" from a record whose bytes
//! are not explained. More than `CAP` values collapse to top, which is never
//! a proof. Everything is bounded: the graph by `MAX_NODES`, the worklist by
//! the lattice height, and the call-site rounds likewise.
use alloc::{collections::BTreeMap, string::String, vec::Vec};

/// Distinct values a set holds before it collapses to top.
pub(crate) const CAP: usize = 4;
/// Product nodes (record, scope) over all procedures.
pub(crate) const MAX_NODES: usize = 1 << 19;
/// The caller's state, inside a procedure's own analysis.
pub(crate) const ENTRY: u32 = 0;
/// Nothing has written the state since the shape start.
pub(crate) const UNDEFINED: u32 = 1;
const NONE: u32 = u32::MAX;

/// The value a writer record (an E2/E0 selector or an 82 buffer) leaves.
pub(crate) fn written(rec: u32) -> u32 {
    2 + 2 * rec
}
/// The value after unexplained bytes at a record.
pub(crate) fn unknown(rec: u32) -> u32 {
    3 + 2 * rec
}
/// A writer record (`Ok`) or the record of unexplained bytes (`Err`).
pub(crate) fn origin(value: u32) -> Option<core::result::Result<u32, u32>> {
    let v = value.checked_sub(2)?;
    Some(if v % 2 == 0 { Ok(v / 2) } else { Err(v / 2) })
}

/// A set of at most `CAP` values, or top.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Set {
    len: u8,
    top: bool,
    v: [u32; CAP],
}
impl Set {
    pub const BOTTOM: Set = Set {
        len: 0,
        top: false,
        v: [0; CAP],
    };
    pub const TOP: Set = Set {
        len: 0,
        top: true,
        v: [0; CAP],
    };
    pub fn one(x: u32) -> Self {
        let mut s = Self::BOTTOM;
        s.insert(x);
        s
    }
    pub fn is_bottom(&self) -> bool {
        !self.top && self.len == 0
    }
    pub fn is_top(&self) -> bool {
        self.top
    }
    /// The values, sorted; empty for top.
    pub fn values(&self) -> &[u32] {
        &self.v[..self.len as usize]
    }
    pub fn contains(&self, x: u32) -> bool {
        self.values().contains(&x)
    }
    fn insert(&mut self, x: u32) -> bool {
        if self.top || self.contains(x) {
            return false;
        }
        if self.len as usize == CAP {
            *self = Self::TOP;
            return true;
        }
        let at = self.values().partition_point(|y| *y < x);
        let n = self.len as usize;
        self.v.copy_within(at..n, at + 1);
        self.v[at] = x;
        self.len += 1;
        true
    }
    /// Add `other`'s values; true when this set grew.
    pub fn join(&mut self, other: &Set) -> bool {
        if self.top {
            return false;
        }
        if other.top {
            *self = Self::TOP;
            return true;
        }
        let mut changed = false;
        for x in other.values() {
            changed |= self.insert(*x);
        }
        changed
    }
    /// This set with `ENTRY` replaced by `entry`'s values.
    pub fn subst(&self, entry: &Set) -> Set {
        if self.top {
            return Self::TOP;
        }
        let mut out = Self::BOTTOM;
        for x in self.values() {
            if *x == ENTRY {
                out.join(entry);
            } else {
                out.insert(*x);
            }
        }
        out
    }
}

/// How one record moves control. Targets are record indices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// Continue at each target (fall-through, branches, x86 resumes).
    Go(Vec<u32>),
    /// Call a block (its entry record, `None` when the target is not a
    /// record start), then continue at `ret`.
    Call {
        callee: Option<u32>,
        ret: Option<u32>,
    },
    /// 1E: padding while a 38 scope covers it, otherwise a return.
    Pad { next: Option<u32> },
    /// 00: return.
    Return,
    /// 38: the scope extends to CODE offset `end`.
    Scope { end: u32, next: Option<u32> },
    /// Bytes the inventory does not explain: the state after them is unknown.
    Unexplained(Vec<u32>),
}
#[derive(Clone, Debug)]
struct Node {
    rec: u32,
    proc: u32,
    /// CODE offset the innermost 38 scope of this frame ends at (0: none).
    scope: u32,
    /// Range of `Graph::succ`.
    succ: (u32, u32),
    /// Callee procedure (`NONE` when the target is not a record start) and
    /// the node the call returns to (`NONE` without a next record).
    call: Option<(u32, u32)>,
    exit: bool,
    unexplained: bool,
}
/// The product graph of a shape, independent of what is being proved.
#[derive(Clone, Debug, Default)]
pub(crate) struct Graph {
    nodes: Vec<Node>,
    succ: Vec<u32>,
    /// Entry node of each procedure; procedure 0 starts at record 0.
    entries: Vec<u32>,
    /// Call nodes calling each procedure.
    callers: Vec<Vec<u32>>,
    /// Call nodes inside each procedure.
    calls: Vec<Vec<u32>>,
    /// Nodes of each record.
    at: Vec<Vec<u32>>,
}
struct Builder<'a> {
    g: Graph,
    index: BTreeMap<(u32, u32, u32), u32>,
    procs: BTreeMap<u32, u32>,
    entry_recs: Vec<u32>,
    work: Vec<u32>,
    steps: &'a [Step],
}
impl Builder<'_> {
    /// Node for (procedure, record, scope), created and queued on first use.
    fn node(&mut self, proc: u32, rec: u32, scope: u32) -> core::result::Result<u32, String> {
        if rec as usize >= self.steps.len() {
            return Err("a path leaves CODE".into());
        }
        if let Some(i) = self.index.get(&(proc, rec, scope)) {
            return Ok(*i);
        }
        if self.g.nodes.len() >= MAX_NODES {
            return Err("its control flow has too many paths to follow".into());
        }
        let i = self.g.nodes.len() as u32;
        self.g.nodes.push(Node {
            rec,
            proc,
            scope,
            succ: (0, 0),
            call: None,
            exit: false,
            unexplained: false,
        });
        self.g.at[rec as usize].push(i);
        self.index.insert((proc, rec, scope), i);
        self.work.push(i);
        Ok(i)
    }
    fn procedure(&mut self, entry: u32) -> u32 {
        if let Some(p) = self.procs.get(&entry) {
            return *p;
        }
        let p = self.entry_recs.len() as u32;
        self.procs.insert(entry, p);
        self.entry_recs.push(entry);
        self.g.callers.push(Vec::new());
        self.g.calls.push(Vec::new());
        p
    }
    /// Successors of node `i` in procedure `p`.
    fn expand(&mut self, p: u32, i: u32, offsets: &[u32]) -> core::result::Result<(), String> {
        let (rec, scope) = (self.g.nodes[i as usize].rec, self.g.nodes[i as usize].scope);
        let mut targets: Vec<(u32, u32)> = Vec::new();
        match &self.steps[rec as usize] {
            Step::Go(ts) => targets.extend(ts.iter().map(|t| (*t, scope))),
            Step::Unexplained(ts) => {
                self.g.nodes[i as usize].unexplained = true;
                targets.extend(ts.iter().map(|t| (*t, scope)));
            }
            Step::Pad { next } => {
                if scope == 0 || offsets[rec as usize] >= scope {
                    self.g.nodes[i as usize].exit = true;
                } else {
                    targets.extend(next.map(|t| (t, scope)));
                }
            }
            Step::Return => self.g.nodes[i as usize].exit = true,
            Step::Scope { end, next } => {
                let s = if *end > offsets[rec as usize] {
                    scope.max(*end)
                } else {
                    scope
                };
                targets.extend(next.map(|t| (t, s)));
            }
            Step::Call { callee, ret } => {
                let c = match callee {
                    Some(t) if (*t as usize) < self.steps.len() => self.procedure(*t),
                    _ => NONE,
                };
                let r = match ret {
                    Some(t) => self.node(p, *t, scope)?,
                    None => NONE,
                };
                self.g.nodes[i as usize].call = Some((c, r));
                self.g.calls[p as usize].push(i);
                if c != NONE {
                    self.g.callers[c as usize].push(i);
                }
            }
        }
        let start = self.g.succ.len() as u32;
        for (t, s) in targets {
            let j = self.node(p, t, s)?;
            self.g.succ.push(j);
        }
        self.g.nodes[i as usize].succ = (start, self.g.succ.len() as u32 - start);
        Ok(())
    }
}
impl Graph {
    /// Build from one step and CODE offset per record, starting at record 0.
    pub fn build(steps: &[Step], offsets: &[u32]) -> core::result::Result<Self, String> {
        let n = steps.len();
        if offsets.len() != n || n == 0 {
            return Err("the shape has no records to follow".into());
        }
        let mut b = Builder {
            g: Graph {
                at: alloc::vec![Vec::new(); n],
                ..Self::default()
            },
            index: BTreeMap::new(),
            procs: BTreeMap::new(),
            entry_recs: Vec::new(),
            work: Vec::new(),
            steps,
        };
        b.procedure(0);
        // Every node is expanded once; `node` caps how many there are.
        let mut p = 0;
        while p < b.entry_recs.len() {
            let entry = b.node(p as u32, b.entry_recs[p], 0)?;
            b.g.entries.push(entry);
            while let Some(i) = b.work.pop() {
                b.expand(p as u32, i, offsets)?;
            }
            p += 1;
        }
        Ok(b.g)
    }
    #[cfg(test)]
    pub fn nodes(&self) -> usize {
        self.nodes.len()
    }
    #[cfg(test)]
    pub fn procedures(&self) -> usize {
        self.entries.len()
    }
    /// The values that may hold on entry to each of `records`, for a state
    /// that `gen` writes: `gen(record)` is the value a writer record leaves.
    pub fn solve(
        &self,
        gen: &dyn Fn(u32) -> Option<u32>,
        records: &[u32],
    ) -> core::result::Result<Vec<Set>, String> {
        let (n, procs) = (self.nodes.len(), self.entries.len());
        let mut input = alloc::vec![Set::BOTTOM; n];
        let mut summary = alloc::vec![Set::BOTTOM; procs];
        let mut queued = alloc::vec![false; n];
        let mut work: Vec<u32> = Vec::new();
        for e in &self.entries {
            input[*e as usize] = Set::one(ENTRY);
            queued[*e as usize] = true;
            work.push(*e);
        }
        // A node's input grows at most CAP + 1 times and a summary change
        // requeues each caller at most CAP + 1 times, so pops stay below this.
        let mut budget = (CAP + 3) * 2 * n + procs + 64;
        while let Some(i) = work.pop() {
            let i = i as usize;
            queued[i] = false;
            if budget == 0 {
                return Err("its control flow did not settle within the analysis bound".into());
            }
            budget -= 1;
            let node = &self.nodes[i];
            let x = input[i];
            let out = if node.unexplained {
                Set::one(unknown(node.rec))
            } else if let Some(v) = gen(node.rec) {
                Set::one(v)
            } else {
                x
            };
            let mut flow = |j: usize, v: &Set, input: &mut Vec<Set>| {
                if input[j].join(v) && !queued[j] {
                    queued[j] = true;
                    work.push(j as u32);
                }
            };
            if let Some((c, r)) = node.call {
                if r != NONE {
                    let v = if c == NONE {
                        Set::one(unknown(node.rec))
                    } else {
                        summary[c as usize].subst(&x)
                    };
                    flow(r as usize, &v, &mut input);
                }
            }
            let (s, len) = node.succ;
            for j in &self.succ[s as usize..(s + len) as usize] {
                flow(*j as usize, &out, &mut input);
            }
            if node.exit && summary[node.proc as usize].join(&out) {
                for c in &self.callers[node.proc as usize] {
                    let c = *c as usize;
                    if !input[c].is_bottom() && !queued[c] {
                        queued[c] = true;
                        work.push(c as u32);
                    }
                }
            }
        }
        // What each procedure is entered with: the shape start for procedure
        // 0, joined with every reachable call site's state. Each round that
        // changes something grows one of these sets.
        let mut actual = alloc::vec![Set::BOTTOM; procs];
        actual[0] = Set::one(UNDEFINED);
        let mut settled = false;
        for _ in 0..procs * (CAP + 2) + 2 {
            let mut changed = false;
            for q in 0..procs {
                if actual[q].is_bottom() {
                    continue;
                }
                let a = actual[q];
                for c in &self.calls[q] {
                    if let Some((callee, _)) = self.nodes[*c as usize].call {
                        if callee != NONE {
                            let v = input[*c as usize].subst(&a);
                            changed |= actual[callee as usize].join(&v);
                        }
                    }
                }
            }
            if !changed {
                settled = true;
                break;
            }
        }
        if !settled {
            return Err("its call sites did not settle within the analysis bound".into());
        }
        Ok(records
            .iter()
            .map(|r| {
                let mut out = Set::BOTTOM;
                for i in self.at.get(*r as usize).into_iter().flatten() {
                    let node = &self.nodes[*i as usize];
                    let a = &actual[node.proc as usize];
                    if !a.is_bottom() {
                        out.join(&input[*i as usize].subst(a));
                    }
                }
                out
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use Step::*;
    fn offsets(n: usize) -> Vec<u32> {
        (0..n as u32).map(|i| 10 * i).collect()
    }
    /// Records listed in `writers` write their own value.
    fn solve(steps: &[Step], writers: &[u32], at: &[u32]) -> Vec<Set> {
        let g = Graph::build(steps, &offsets(steps.len())).unwrap();
        g.solve(&|r| writers.contains(&r).then(|| written(r)), at)
            .unwrap()
    }
    fn set(values: &[u32]) -> Set {
        let mut s = Set::BOTTOM;
        for v in values {
            s.join(&Set::one(*v));
        }
        s
    }

    #[test]
    fn sets_collapse_to_top_and_substitute_the_entry() {
        let mut s = Set::BOTTOM;
        for v in 0..CAP as u32 {
            assert!(s.join(&Set::one(10 + v)));
        }
        assert!(!s.is_top());
        assert!(s.join(&Set::one(99)) && s.is_top());
        assert!(!s.join(&Set::one(5)));
        assert_eq!(set(&[ENTRY, 7]).subst(&set(&[3, 4])), set(&[3, 4, 7]));
        assert_eq!(set(&[7]).subst(&Set::TOP), set(&[7]));
        assert!(set(&[ENTRY]).subst(&Set::TOP).is_top());
        assert_eq!(origin(written(5)), Some(Ok(5)));
        assert_eq!(origin(unknown(5)), Some(Err(5)));
        assert_eq!(origin(UNDEFINED), None);
    }

    #[test]
    fn loops_reach_a_fixed_point() {
        // 0 writes; 1 loops back to itself and on; 2 rewrites inside a loop
        // back to 1, so 1 sees both writers and 3 only the second.
        let steps = [Go(vec![1]), Go(vec![1, 2]), Go(vec![1, 3]), Return];
        let got = solve(&steps, &[0, 2], &[1, 3]);
        assert_eq!(got, [set(&[written(0), written(2)]), set(&[written(2)])]);
        // Without the rewrite the loop keeps the first writer.
        let got = solve(&steps, &[0], &[1, 3]);
        assert_eq!(got, [set(&[written(0)]); 2]);
    }

    #[test]
    fn calls_are_context_sensitive() {
        // 0 writes A, calls 6 from 1 and from 4 after 3 writes B. Block 6
        // returns untouched, so 2 sees A and 5 sees B; 6 itself sees both.
        let steps = [
            Go(vec![1]),
            Call {
                callee: Some(6),
                ret: Some(2),
            },
            Go(vec![3]),
            Go(vec![4]),
            Call {
                callee: Some(6),
                ret: Some(5),
            },
            Return,
            Pad { next: Some(7) },
            Return,
        ];
        let got = solve(&steps, &[0, 3], &[2, 5, 6]);
        assert_eq!(
            got,
            [
                set(&[written(0)]),
                set(&[written(3)]),
                set(&[written(0), written(3)])
            ]
        );
    }

    #[test]
    fn scopes_turn_returns_into_padding() {
        // 0 calls 2 (whose 1E at 3 returns), then 1 opens a scope over 3 and
        // falls into the block again: 3 is padding, 4 writes, 5 returns.
        let steps = [
            Call {
                callee: Some(2),
                ret: Some(1),
            },
            Scope {
                end: 35,
                next: Some(2),
            },
            Go(vec![3]),
            Pad { next: Some(4) },
            Go(vec![5]),
            Pad { next: None },
            Return,
        ];
        let g = Graph::build(&steps, &offsets(steps.len())).unwrap();
        // Record 4 is reached only under the scope, never by the call.
        let got = g
            .solve(&|r| (r == 4).then(|| written(4)), &[1, 2, 4, 5])
            .unwrap();
        assert_eq!(
            got,
            [
                set(&[UNDEFINED]),
                set(&[UNDEFINED]),
                set(&[UNDEFINED]),
                set(&[written(4)])
            ]
        );
        assert_eq!(g.procedures(), 2);
    }

    #[test]
    fn unexplained_bytes_and_bad_calls_are_unknown() {
        let steps = [
            Unexplained(vec![1]),
            Call {
                callee: None,
                ret: Some(2),
            },
            Return,
        ];
        let got = solve(&steps, &[], &[1, 2]);
        assert_eq!(got, [set(&[unknown(0)]), set(&[unknown(1)])]);
    }

    #[test]
    fn recursion_terminates() {
        // A block that only calls itself never returns: what follows its
        // calls is unreachable. Mutual recursion with writers settles.
        let steps = [
            Call {
                callee: Some(2),
                ret: Some(1),
            },
            Return,
            Call {
                callee: Some(2),
                ret: Some(3),
            },
            Return,
        ];
        assert_eq!(solve(&steps, &[], &[1, 3]), [Set::BOTTOM; 2]);
        let steps = [
            Call {
                callee: Some(2),
                ret: Some(1),
            },
            Return,
            Go(vec![3, 5]),
            Call {
                callee: Some(6),
                ret: Some(4),
            },
            Return,
            Return,
            Go(vec![7, 9]),
            Call {
                callee: Some(2),
                ret: Some(8),
            },
            Return,
            Return,
        ];
        let got = solve(&steps, &[6], &[1, 2, 6]);
        assert_eq!(got[0], set(&[UNDEFINED, written(6)]));
        assert_eq!(got[1], set(&[UNDEFINED, written(6)]));
        assert_eq!(got[2], set(&[UNDEFINED, written(6)]));
    }

    #[test]
    fn adversarial_graphs_stay_within_their_bounds() {
        // Every record branches to the next three and back to the start, and
        // every third writes: sets overflow to top and the solver settles.
        let n = 3000u32;
        let steps: Vec<Step> = (0..n)
            .map(|i| Go((1..4).map(|d| (i + d) % n).chain([0]).collect()))
            .collect();
        let writers: Vec<u32> = (0..n).step_by(3).collect();
        let got = solve(&steps, &writers, &[0, n - 1]);
        assert!(got.iter().all(|s| s.is_top()));
        // A chain of blocks, each calling the next and then the first: a
        // thousand mutually recursive procedures, settled within bounds.
        let n = 4000u32;
        let steps: Vec<Step> = (0..n)
            .map(|i| match i % 4 {
                0 => Go(vec![i + 1, i + 3]),
                1 => Call {
                    callee: Some((i + 3) % n),
                    ret: Some(i + 1),
                },
                2 => Call {
                    callee: Some(0),
                    ret: Some(i + 1),
                },
                _ => Return,
            })
            .collect();
        let g = Graph::build(&steps, &offsets(steps.len())).unwrap();
        assert_eq!(g.procedures(), 1000);
        assert_eq!(g.nodes(), 4000);
        let got = g
            .solve(&|r| (r % 7 == 0).then(|| written(r)), &[0, 1, 2])
            .unwrap();
        assert!(got.iter().all(|s| !s.is_bottom()));
        // Nested scopes ending ever later, with a path back to the start
        // under each: the product graph would be quadratic, so it stops at
        // its node cap with a refusal instead.
        let n = 2000u32;
        let steps: Vec<Step> = (0..n)
            .map(|i| {
                if i % 2 == 0 {
                    Scope {
                        end: 100_000 + i,
                        next: Some(i + 1),
                    }
                } else {
                    Go(vec![(i + 1) % n, 0])
                }
            })
            .collect();
        let e = Graph::build(&steps, &offsets(steps.len())).unwrap_err();
        assert!(e.contains("too many paths"), "{e}");
        // Targets outside the graph are refused, not followed.
        assert!(Graph::build(&[Go(vec![5])], &[0]).is_err());
        assert!(Graph::build(&[], &[]).is_err());
    }
}
