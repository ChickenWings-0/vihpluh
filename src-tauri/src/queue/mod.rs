//! The play queue (ARCHITECTURE §8.1): what plays next, right now. vihpluh owns it; mpv's own
//! playlist only ever holds the current file (D4).
//!
//! This module is pure state: no mpv, no disk. Every operation returns what the core has to do
//! (load an item, restart, stop…), which keeps the whole rule set unit-testable.

pub mod session;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::config::schema::AtQueueEnd;

/// A Previous press restarts the current item if it has played longer than this (§8.1).
pub const PREVIOUS_RESTART_AFTER_S: f64 = 5.0;
/// After this many consecutive unplayable items, auto-advance stops (§6.4).
pub const MAX_CONSECUTIVE_ERRORS: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Repeat {
    #[default]
    Off,
    One,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Origin {
    User,
    Folder,
    Playlist,
    Session,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QueueItem {
    pub id: String,
    pub source: String,
    pub is_url: bool,
    pub title: String,
    pub duration: Option<f64>,
    pub media_key: Option<String>,
    pub origin: Origin,
    pub origin_ref: Option<String>,
    /// Why the item could not be played, once it failed.
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QueueState {
    #[ts(type = "number")]
    pub rev: u64,
    pub items: Vec<QueueItem>,
    pub current_id: Option<String>,
    pub repeat: Repeat,
    pub shuffle: bool,
    pub stop_after_current: bool,
    /// Whether Next / Previous have somewhere to go (buttons are disabled otherwise).
    pub can_next: bool,
    pub can_previous: bool,
}

/// A source about to join the queue.
#[derive(Debug, Clone, PartialEq)]
pub struct NewItem {
    pub source: String,
    pub origin: Origin,
    pub origin_ref: Option<String>,
}

impl NewItem {
    pub fn user(source: impl Into<String>) -> Self {
        Self { source: source.into(), origin: Origin::User, origin_ref: None }
    }
}

/// What the core must do after a queue operation.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Load this item (it is now current).
    Load(QueueItem),
    /// Start the current item again from the beginning.
    Restart,
    /// Keep the last frame on screen (`at_queue_end = hold`).
    Hold,
    /// Stop playback and go idle.
    Stop,
    /// Quit the app (`at_queue_end = quit`).
    Quit,
    /// Stop after repeated failures; the list names what failed.
    GiveUp(Vec<String>),
    /// Nothing to do.
    Nothing,
}

/// The title shown before the file reports its own: the file name, or the URL.
pub fn title_for(source: &str) -> String {
    if crate::cli::is_url(source) {
        return source.to_string();
    }
    std::path::Path::new(source)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| source.to_string())
}

/// Small deterministic generator for shuffle order and ids (no dependency, seedable in tests).
#[derive(Debug, Clone)]
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next_u64() % n as u64) as usize }
    }

    /// A random v4-format UUID string.
    fn uuid(&mut self) -> String {
        let (a, b) = (self.next_u64(), self.next_u64());
        let a = (a & 0xFFFF_FFFF_FFFF_0FFF) | 0x0000_0000_0000_4000; // version 4
        let b = (b & 0x3FFF_FFFF_FFFF_FFFF) | 0x8000_0000_0000_0000; // variant 10
        format!(
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            a >> 32,
            (a >> 16) & 0xFFFF,
            a & 0xFFFF,
            b >> 48,
            b & 0xFFFF_FFFF_FFFF
        )
    }
}

#[derive(Debug, Clone)]
pub struct Queue {
    items: Vec<QueueItem>,
    current: Option<String>,
    /// Where the current item was, so Next still works after it was removed.
    current_index: usize,
    repeat: Repeat,
    /// Play order while shuffling (ids); `None` when shuffle is off.
    order: Option<Vec<String>>,
    stop_after_current: bool,
    consecutive_errors: u32,
    failures: Vec<String>,
    rev: u64,
    rng: Rng,
}

impl Queue {
    pub fn new(seed: u64) -> Self {
        Self {
            items: Vec::new(),
            current: None,
            current_index: 0,
            repeat: Repeat::Off,
            order: None,
            stop_after_current: false,
            consecutive_errors: 0,
            failures: Vec::new(),
            rev: 0,
            rng: Rng(seed | 1),
        }
    }

    // ── Reading ─────────────────────────────────────────────────────────────

    pub fn state(&self) -> QueueState {
        QueueState {
            rev: self.rev,
            items: self.items.clone(),
            current_id: self.current.clone(),
            repeat: self.repeat,
            shuffle: self.order.is_some(),
            stop_after_current: self.stop_after_current,
            can_next: self.neighbour(1, self.repeat == Repeat::All).is_some(),
            can_previous: self.current.is_some(),
        }
    }

    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    pub fn current(&self) -> Option<&QueueItem> {
        let id = self.current.as_ref()?;
        self.items.iter().find(|item| &item.id == id)
    }

    pub fn repeat(&self) -> Repeat {
        self.repeat
    }

    fn index_of(&self, id: &str) -> Option<usize> {
        self.items.iter().position(|item| item.id == id)
    }

    /// The ids in play order (shuffle order, or list order).
    fn play_order(&self) -> Vec<String> {
        match &self.order {
            Some(order) => order.clone(),
            None => self.items.iter().map(|i| i.id.clone()).collect(),
        }
    }

    /// The item `step` places away from the current one in play order (1 = next).
    fn neighbour(&self, step: isize, wrap: bool) -> Option<QueueItem> {
        let order = self.play_order();
        if order.is_empty() {
            return None;
        }
        let position = match &self.current {
            Some(id) => match order.iter().position(|o| o == id) {
                Some(position) => position as isize,
                // The current item was removed: continue from where it was.
                None if self.order.is_none() => self.current_index as isize - step.signum().max(0),
                None => -1,
            },
            None => -1,
        };
        let mut target = position + step;
        let len = order.len() as isize;
        if target < 0 || target >= len {
            if !wrap {
                return None;
            }
            target = target.rem_euclid(len);
        }
        let id = &order[target as usize];
        self.items.iter().find(|item| &item.id == id).cloned()
    }

    // ── Changing ────────────────────────────────────────────────────────────

    fn changed(&mut self) {
        self.rev += 1;
    }

    fn make_item(&mut self, new: NewItem) -> QueueItem {
        QueueItem {
            id: self.rng.uuid(),
            is_url: crate::cli::is_url(&new.source),
            title: title_for(&new.source),
            source: new.source,
            duration: None,
            media_key: None,
            origin: new.origin,
            origin_ref: new.origin_ref,
            error: None,
        }
    }

    fn set_current(&mut self, id: &str) -> Option<QueueItem> {
        let index = self.index_of(id)?;
        self.current = Some(id.to_string());
        self.current_index = index;
        self.items.get(index).cloned()
    }

    /// Any manual edit turns a folder-filled queue into an ordinary one (§8.1).
    fn edited_by_user(&mut self) {
        for item in &mut self.items {
            if item.origin == Origin::Folder {
                item.origin = Origin::User;
            }
        }
    }

    fn reshuffle(&mut self) {
        if self.order.is_none() {
            return;
        }
        // The current item first, the rest in random order (Fisher–Yates).
        let mut rest: Vec<String> = self
            .items
            .iter()
            .map(|i| i.id.clone())
            .filter(|id| Some(id) != self.current.as_ref())
            .collect();
        for i in (1..rest.len()).rev() {
            let j = self.rng.below(i + 1);
            rest.swap(i, j);
        }
        let mut order: Vec<String> = self.current.iter().cloned().collect();
        order.extend(rest);
        self.order = Some(order);
    }

    /// Replaces the whole queue and makes `start` current. Returns the item to load.
    pub fn replace(&mut self, items: Vec<NewItem>, start: usize) -> Decision {
        let items: Vec<QueueItem> = items.into_iter().map(|n| self.make_item(n)).collect();
        self.items = items;
        self.current = None;
        self.current_index = 0;
        self.stop_after_current = false;
        self.consecutive_errors = 0;
        self.failures.clear();
        let decision = match self.items.get(start.min(self.items.len().saturating_sub(1))) {
            Some(item) => {
                let id = item.id.clone();
                self.set_current(&id).map_or(Decision::Nothing, Decision::Load)
            }
            None => Decision::Nothing,
        };
        self.reshuffle();
        self.changed();
        decision
    }

    /// Inserts items at `at` (end when `None`). Never interrupts; if nothing is current, the
    /// first new item becomes current and is loaded. Returns the new ids and the decision.
    pub fn add(&mut self, items: Vec<NewItem>, at: Option<usize>) -> (Vec<String>, Decision) {
        let new: Vec<QueueItem> = items.into_iter().map(|n| self.make_item(n)).collect();
        let ids: Vec<String> = new.iter().map(|i| i.id.clone()).collect();
        let at = at.unwrap_or(self.items.len()).min(self.items.len());
        self.items.splice(at..at, new);
        if let Some(current) = self.current.clone() {
            self.current_index = self.index_of(&current).unwrap_or(self.current_index);
        }
        self.edited_by_user();
        if let Some(order) = &mut self.order {
            // New items join the end of the shuffle order, in random positions among themselves.
            let mut tail = ids.clone();
            for i in (1..tail.len()).rev() {
                let j = self.rng.below(i + 1);
                tail.swap(i, j);
            }
            order.extend(tail);
        }
        let decision = if self.current.is_none() {
            ids.first()
                .cloned()
                .and_then(|id| self.set_current(&id))
                .map_or(Decision::Nothing, Decision::Load)
        } else {
            Decision::Nothing
        };
        self.changed();
        (ids, decision)
    }

    /// Inserts right after the current item (in list and play order).
    pub fn play_next(&mut self, items: Vec<NewItem>) -> (Vec<String>, Decision) {
        let at = match &self.current {
            Some(id) => self.index_of(id).map_or(self.current_index, |i| i + 1),
            None => self.items.len(),
        };
        let shuffled = self.order.is_some();
        let (ids, decision) = self.add(items, Some(at));
        if shuffled {
            // Move the new ids to right after the current one in play order.
            if let (Some(order), Some(current)) = (&mut self.order, &self.current) {
                order.retain(|id| !ids.contains(id));
                let pos = order.iter().position(|id| id == current).map_or(0, |p| p + 1);
                for (offset, id) in ids.iter().enumerate() {
                    order.insert(pos + offset, id.clone());
                }
            }
        }
        (ids, decision)
    }

    /// Removes items. Removing the current item does not stop playback; Next then continues
    /// from where it was.
    pub fn remove(&mut self, ids: &[String]) {
        if let Some(current) = self.current.clone() {
            self.current_index = self.index_of(&current).unwrap_or(self.current_index);
            // Keep the index pointing at the slot after the removed current item.
            let removed_before = self.items[..self.current_index.min(self.items.len())]
                .iter()
                .filter(|i| ids.contains(&i.id))
                .count();
            self.current_index -= removed_before;
        }
        self.items.retain(|item| !ids.contains(&item.id));
        if let Some(order) = &mut self.order {
            order.retain(|id| !ids.contains(id));
        }
        self.edited_by_user();
        self.changed();
    }

    /// Moves `ids` (kept in their current relative order) so the first lands at `to_index`
    /// of the list without them.
    pub fn move_items(&mut self, ids: &[String], to_index: usize) {
        let (moving, mut rest): (Vec<QueueItem>, Vec<QueueItem>) =
            self.items.drain(..).partition(|item| ids.contains(&item.id));
        let at = to_index.min(rest.len());
        rest.splice(at..at, moving);
        self.items = rest;
        if let Some(current) = self.current.clone() {
            self.current_index = self.index_of(&current).unwrap_or(self.current_index);
        }
        self.edited_by_user();
        self.changed();
    }

    /// Clears the queue; with `keep_current` the playing item stays.
    pub fn clear(&mut self, keep_current: bool) {
        let keep = if keep_current { self.current.clone() } else { None };
        self.items.retain(|item| Some(&item.id) == keep.as_ref());
        if !keep_current {
            self.current = None;
        }
        self.current_index = 0;
        if self.order.is_some() {
            self.order = Some(self.items.iter().map(|i| i.id.clone()).collect());
        }
        self.edited_by_user();
        self.changed();
    }

    /// Plays a specific item (double-click in the queue panel).
    pub fn select(&mut self, id: &str) -> Decision {
        self.consecutive_errors = 0;
        let decision = self.set_current(id).map_or(Decision::Nothing, Decision::Load);
        self.changed();
        decision
    }

    /// The user pressed Next. Wraps only with repeat-all; at the end it does nothing.
    pub fn go_next(&mut self) -> Decision {
        self.consecutive_errors = 0;
        match self.neighbour(1, self.repeat == Repeat::All) {
            Some(item) => {
                let decision = self.set_current(&item.id).map_or(Decision::Nothing, Decision::Load);
                self.changed();
                decision
            }
            None => Decision::Nothing,
        }
    }

    /// The user pressed Previous: restart if the item has played a while, else go back.
    pub fn go_previous(&mut self, position: f64) -> Decision {
        if self.current.is_none() {
            return Decision::Nothing;
        }
        if position > PREVIOUS_RESTART_AFTER_S {
            return Decision::Restart;
        }
        self.consecutive_errors = 0;
        match self.neighbour(-1, self.repeat == Repeat::All) {
            Some(item) => {
                let decision = self.set_current(&item.id).map_or(Decision::Nothing, Decision::Load);
                self.changed();
                decision
            }
            None => Decision::Restart,
        }
    }

    /// The current item played to its end.
    pub fn on_ended(&mut self, at_queue_end: AtQueueEnd) -> Decision {
        if self.current.is_none() {
            return Decision::Nothing;
        }
        if self.stop_after_current {
            self.stop_after_current = false;
            self.changed();
            return end_decision(at_queue_end);
        }
        if self.repeat == Repeat::One {
            return Decision::Restart;
        }
        match self.neighbour(1, self.repeat == Repeat::All) {
            Some(item) => {
                let decision = self.set_current(&item.id).map_or(Decision::Nothing, Decision::Load);
                self.changed();
                decision
            }
            None => end_decision(at_queue_end),
        }
    }

    /// The current item failed to load: mark it and skip to the next one, unless too many
    /// failed in a row.
    pub fn on_error(&mut self, message: &str, at_queue_end: AtQueueEnd) -> Decision {
        let Some(id) = self.current.clone() else { return Decision::Nothing };
        if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
            item.error = Some(message.to_string());
            self.failures.push(item.title.clone());
        }
        self.consecutive_errors += 1;
        self.changed();
        if self.consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
            self.consecutive_errors = 0;
            return Decision::GiveUp(std::mem::take(&mut self.failures));
        }
        // Repeat-one on a broken file would loop forever; skip regardless.
        let wrap = self.repeat == Repeat::All;
        match self.neighbour(1, wrap) {
            Some(item) if item.error.is_none() || item.id != id => {
                let decision = self.set_current(&item.id).map_or(Decision::Nothing, Decision::Load);
                self.changed();
                decision
            }
            _ => {
                self.failures.clear();
                match end_decision(at_queue_end) {
                    // Nothing to hold on: the failed item has no frame.
                    Decision::Hold => Decision::Stop,
                    other => other,
                }
            }
        }
    }

    /// The current item loaded: it is playable again and the error streak ends.
    pub fn on_loaded(
        &mut self,
        title: Option<&str>,
        duration: Option<f64>,
        media_key: Option<&str>,
    ) {
        self.consecutive_errors = 0;
        self.failures.clear();
        let Some(id) = self.current.clone() else { return };
        if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
            let mut changed = item.error.take().is_some();
            if let Some(d) = duration.filter(|d| *d > 0.0) {
                changed |= item.duration != Some(d);
                item.duration = Some(d);
            }
            if let Some(key) = media_key {
                changed |= item.media_key.as_deref() != Some(key);
                item.media_key = Some(key.to_string());
            }
            if let Some(title) = title.filter(|t| !t.is_empty()) {
                // URLs get the stream's own title; files keep their file name.
                if item.is_url && item.title != title {
                    item.title = title.to_string();
                    changed = true;
                }
            }
            if changed {
                self.changed();
            }
        }
    }

    pub fn set_repeat(&mut self, repeat: Repeat) {
        self.repeat = repeat;
        self.changed();
    }

    /// Shuffle builds one random play order (current first) and keeps it until switched off;
    /// switching off continues in list order from the current item.
    pub fn set_shuffle(&mut self, on: bool) {
        if on == self.order.is_some() {
            return;
        }
        self.order = if on { Some(Vec::new()) } else { None };
        self.reshuffle();
        self.changed();
    }

    pub fn set_stop_after_current(&mut self, on: bool) {
        self.stop_after_current = on;
        self.changed();
    }

    /// Restores a saved queue without loading anything (session restore).
    pub fn restore(
        &mut self,
        items: Vec<QueueItem>,
        current: Option<String>,
        repeat: Repeat,
        shuffle: bool,
    ) {
        self.items = items;
        self.current = current.filter(|id| self.items.iter().any(|i| &i.id == id));
        self.current_index = self.current.as_deref().and_then(|id| self.index_of(id)).unwrap_or(0);
        self.repeat = repeat;
        self.order = shuffle.then(Vec::new);
        self.reshuffle();
        self.changed();
    }
}

fn end_decision(at_queue_end: AtQueueEnd) -> Decision {
    match at_queue_end {
        AtQueueEnd::Hold => Decision::Hold,
        AtQueueEnd::Stop => Decision::Stop,
        AtQueueEnd::Quit => Decision::Quit,
    }
}

/// Queue contents for opening exactly one local file with `playback.auto_queue_folder`: the
/// folder's media files in natural order, flagged `folder`, and the index of the opened file.
/// `None` when the file is not in the listing (then open just the file).
pub fn folder_fill(file: &str) -> Option<(Vec<NewItem>, usize)> {
    let path = std::path::Path::new(file);
    let dir = path.parent()?;
    let listing = crate::media::files::list_folder(dir);
    // The listing is of this file's own folder, so its name identifies it (and separator
    // spelling — `/` or `\` on Windows — cannot get in the way).
    let name = path.file_name()?;
    let index = listing.iter().position(|p| p.file_name().is_some_and(|n| same_name(n, name)))?;
    let folder = dir.display().to_string();
    let items = listing
        .into_iter()
        .map(|p| NewItem {
            source: p.display().to_string(),
            origin: Origin::Folder,
            origin_ref: Some(folder.clone()),
        })
        .collect();
    Some((items, index))
}

fn same_name(a: &std::ffi::OsStr, b: &std::ffi::OsStr) -> bool {
    if cfg!(windows) {
        a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())
    } else {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue_of(names: &[&str]) -> (Queue, Decision) {
        let mut q = Queue::new(42);
        let d = q.replace(names.iter().map(|n| NewItem::user(*n)).collect(), 0);
        (q, d)
    }

    fn current(q: &Queue) -> String {
        q.current().map(|i| i.source.clone()).unwrap_or_default()
    }

    fn loaded(d: &Decision) -> String {
        match d {
            Decision::Load(item) => item.source.clone(),
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn replace_plays_the_first_item() {
        let (q, d) = queue_of(&["a", "b", "c"]);
        assert_eq!(loaded(&d), "a");
        assert_eq!(current(&q), "a");
        let state = q.state();
        assert_eq!(state.items.len(), 3);
        assert!(state.can_next);
        assert!(state.items.iter().all(|i| i.origin == Origin::User && !i.is_url));
        assert_eq!(state.items[0].title, "a");
    }

    #[test]
    fn replace_can_start_mid_list() {
        let mut q = Queue::new(1);
        let d = q.replace(vec![NewItem::user("a"), NewItem::user("b")], 1);
        assert_eq!(loaded(&d), "b");
    }

    #[test]
    fn ids_are_unique_v4_uuids() {
        let (q, _) = queue_of(&["a", "b", "c", "d"]);
        let ids: Vec<&str> = q.items().iter().map(|i| i.id.as_str()).collect();
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), ids.len());
        for id in ids {
            assert_eq!(id.len(), 36);
            assert_eq!(id.as_bytes()[14], b'4', "{id}");
            assert!(matches!(id.as_bytes()[19], b'8' | b'9' | b'a' | b'b'), "{id}");
        }
    }

    #[test]
    fn next_and_previous_walk_the_list() {
        let (mut q, _) = queue_of(&["a", "b", "c"]);
        assert_eq!(loaded(&q.go_next()), "b");
        assert_eq!(loaded(&q.go_next()), "c");
        assert_eq!(q.go_next(), Decision::Nothing, "no wrap without repeat-all");
        assert!(!q.state().can_next);
        assert_eq!(loaded(&q.go_previous(1.0)), "b");
        assert_eq!(q.go_previous(30.0), Decision::Restart, "more than 5 s in: restart");
        assert_eq!(loaded(&q.go_previous(0.0)), "a");
        assert_eq!(q.go_previous(0.0), Decision::Restart, "at the first item: restart");
    }

    #[test]
    fn auto_advance_and_queue_end() {
        let (mut q, _) = queue_of(&["a", "b"]);
        assert_eq!(loaded(&q.on_ended(AtQueueEnd::Hold)), "b");
        assert_eq!(q.on_ended(AtQueueEnd::Hold), Decision::Hold);
        assert_eq!(q.on_ended(AtQueueEnd::Stop), Decision::Stop);
        assert_eq!(q.on_ended(AtQueueEnd::Quit), Decision::Quit);
    }

    #[test]
    fn repeat_one_and_all() {
        let (mut q, _) = queue_of(&["a", "b"]);
        q.set_repeat(Repeat::One);
        assert_eq!(q.on_ended(AtQueueEnd::Hold), Decision::Restart);
        assert_eq!(loaded(&q.go_next()), "b", "an explicit Next still moves on");
        q.set_repeat(Repeat::All);
        assert_eq!(loaded(&q.on_ended(AtQueueEnd::Hold)), "a", "wraps");
        assert!(q.state().can_next);
        assert_eq!(loaded(&q.go_previous(0.0)), "b", "Previous wraps too");
    }

    #[test]
    fn enqueue_appends_without_interrupting() {
        let (mut q, _) = queue_of(&["a", "b"]);
        let (ids, d) = q.add(vec![NewItem::user("c")], None);
        assert_eq!(d, Decision::Nothing);
        assert_eq!(ids.len(), 1);
        assert_eq!(current(&q), "a");
        assert_eq!(q.items().last().map(|i| i.source.as_str()), Some("c"));
    }

    #[test]
    fn enqueue_into_an_empty_queue_starts_playing() {
        let mut q = Queue::new(1);
        let (_, d) = q.add(vec![NewItem::user("x")], None);
        assert_eq!(loaded(&d), "x");
    }

    #[test]
    fn play_next_inserts_after_current() {
        let (mut q, _) = queue_of(&["a", "b", "c"]);
        q.go_next();
        q.play_next(vec![NewItem::user("x"), NewItem::user("y")]);
        let order: Vec<&str> = q.items().iter().map(|i| i.source.as_str()).collect();
        assert_eq!(order, vec!["a", "b", "x", "y", "c"]);
        assert_eq!(loaded(&q.on_ended(AtQueueEnd::Hold)), "x");
        assert_eq!(loaded(&q.on_ended(AtQueueEnd::Hold)), "y");
        assert_eq!(loaded(&q.on_ended(AtQueueEnd::Hold)), "c");
    }

    #[test]
    fn play_next_also_jumps_the_shuffle_order() {
        let (mut q, _) = queue_of(&["a", "b", "c", "d", "e"]);
        q.set_shuffle(true);
        q.play_next(vec![NewItem::user("x")]);
        assert_eq!(loaded(&q.on_ended(AtQueueEnd::Hold)), "x");
    }

    #[test]
    fn add_at_a_position() {
        let (mut q, _) = queue_of(&["a", "b", "c"]);
        q.add(vec![NewItem::user("x")], Some(1));
        let order: Vec<&str> = q.items().iter().map(|i| i.source.as_str()).collect();
        assert_eq!(order, vec!["a", "x", "b", "c"]);
    }

    #[test]
    fn removing_and_moving_never_interrupt() {
        let (mut q, _) = queue_of(&["a", "b", "c", "d"]);
        q.go_next(); // b
        let ids: Vec<String> = q.items().iter().map(|i| i.id.clone()).collect();
        q.remove(&[ids[0].clone(), ids[2].clone()]); // a, c
        assert_eq!(current(&q), "b");
        let order: Vec<&str> = q.items().iter().map(|i| i.source.as_str()).collect();
        assert_eq!(order, vec!["b", "d"]);
        q.move_items(&[ids[3].clone()], 0); // d to the top
        let order: Vec<&str> = q.items().iter().map(|i| i.source.as_str()).collect();
        assert_eq!(order, vec!["d", "b"]);
        assert_eq!(current(&q), "b");
        assert_eq!(q.go_next(), Decision::Nothing, "b is now last");
    }

    #[test]
    fn removing_the_current_item_continues_from_its_place() {
        let (mut q, _) = queue_of(&["a", "b", "c"]);
        q.go_next(); // b
        let b = q.current().unwrap().id.clone();
        q.remove(&[b]);
        assert_eq!(loaded(&q.on_ended(AtQueueEnd::Hold)), "c");
    }

    #[test]
    fn moving_several_keeps_their_relative_order() {
        let (mut q, _) = queue_of(&["a", "b", "c", "d", "e"]);
        let ids: Vec<String> = q.items().iter().map(|i| i.id.clone()).collect();
        q.move_items(&[ids[3].clone(), ids[1].clone()], 3);
        let order: Vec<&str> = q.items().iter().map(|i| i.source.as_str()).collect();
        assert_eq!(order, vec!["a", "c", "e", "b", "d"]);
    }

    #[test]
    fn clear_keeps_the_current_item() {
        let (mut q, _) = queue_of(&["a", "b", "c"]);
        q.go_next();
        q.clear(true);
        assert_eq!(q.items().len(), 1);
        assert_eq!(current(&q), "b");
        q.clear(false);
        assert!(q.items().is_empty());
        assert!(q.current().is_none());
    }

    #[test]
    fn shuffle_plays_every_item_once_then_resumes_list_order() {
        let names: Vec<String> = (0..12).map(|i| format!("{i:02}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let (mut q, _) = queue_of(&refs);
        q.set_shuffle(true);
        let mut played = vec![current(&q)];
        while let Decision::Load(item) = q.on_ended(AtQueueEnd::Hold) {
            played.push(item.source);
        }
        let mut sorted = played.clone();
        sorted.sort();
        assert_eq!(sorted, names, "each item exactly once");
        assert_ne!(played, names, "not in list order (seeded)");
        assert!(
            q.state().items.iter().map(|i| i.source.clone()).eq(names.iter().cloned()),
            "list order unchanged"
        );

        // Turning shuffle off continues in list order from the current item.
        let last = current(&q);
        q.set_shuffle(false);
        let next = q.go_next();
        let expected = names.iter().position(|n| *n == last).and_then(|i| names.get(i + 1));
        match expected {
            Some(name) => assert_eq!(loaded(&next), *name),
            None => assert_eq!(next, Decision::Nothing),
        }
    }

    #[test]
    fn stop_after_current_fires_once() {
        let (mut q, _) = queue_of(&["a", "b", "c"]);
        q.set_stop_after_current(true);
        assert_eq!(q.on_ended(AtQueueEnd::Hold), Decision::Hold);
        assert!(!q.state().stop_after_current);
        assert_eq!(loaded(&q.on_ended(AtQueueEnd::Hold)), "b");
    }

    #[test]
    fn errors_are_marked_and_skipped_and_three_in_a_row_give_up() {
        let (mut q, _) = queue_of(&["bad1", "bad2", "good", "bad3", "bad4", "bad5", "last"]);
        assert_eq!(loaded(&q.on_error("unrecognized file format", AtQueueEnd::Hold)), "bad2");
        assert_eq!(loaded(&q.on_error("x", AtQueueEnd::Hold)), "good");
        q.on_loaded(None, Some(10.0), None);
        assert_eq!(loaded(&q.on_ended(AtQueueEnd::Hold)), "bad3");
        assert_eq!(loaded(&q.on_error("x", AtQueueEnd::Hold)), "bad4");
        assert_eq!(loaded(&q.on_error("x", AtQueueEnd::Hold)), "bad5");
        match q.on_error("x", AtQueueEnd::Hold) {
            Decision::GiveUp(failed) => assert_eq!(failed, vec!["bad3", "bad4", "bad5"]),
            other => panic!("expected GiveUp, got {other:?}"),
        }
        let errors: Vec<bool> = q.items().iter().map(|i| i.error.is_some()).collect();
        assert_eq!(errors, vec![true, true, false, true, true, true, false]);
    }

    #[test]
    fn an_error_on_the_last_item_stops_instead_of_holding() {
        let (mut q, _) = queue_of(&["only"]);
        assert_eq!(q.on_error("x", AtQueueEnd::Hold), Decision::Stop);
    }

    #[test]
    fn a_failed_item_that_loads_later_loses_its_mark() {
        let (mut q, _) = queue_of(&["a", "b"]);
        q.on_error("x", AtQueueEnd::Hold);
        let a = q.items()[0].id.clone();
        q.select(&a);
        q.on_loaded(Some("A"), Some(60.0), Some("f:1234"));
        let item = &q.items()[0];
        assert_eq!(item.error, None);
        assert_eq!(item.duration, Some(60.0));
        assert_eq!(item.media_key.as_deref(), Some("f:1234"));
        assert_eq!(item.title, "a", "files keep their file name");
    }

    #[test]
    fn urls_take_the_stream_title() {
        let mut q = Queue::new(1);
        q.replace(vec![NewItem::user("https://example.com/live.m3u8")], 0);
        assert!(q.items()[0].is_url);
        q.on_loaded(Some("Live channel"), None, None);
        assert_eq!(q.items()[0].title, "Live channel");
    }

    #[test]
    fn manual_edits_turn_folder_items_into_user_items() {
        let mut q = Queue::new(1);
        q.replace(
            vec![
                NewItem {
                    source: "E1".into(),
                    origin: Origin::Folder,
                    origin_ref: Some("D:/S".into()),
                },
                NewItem {
                    source: "E2".into(),
                    origin: Origin::Folder,
                    origin_ref: Some("D:/S".into()),
                },
            ],
            0,
        );
        assert!(q.items().iter().all(|i| i.origin == Origin::Folder));
        q.go_next();
        assert!(q.items().iter().all(|i| i.origin == Origin::Folder), "playing is not editing");
        let first = q.items()[0].id.clone();
        q.move_items(&[first], 1);
        assert!(q.items().iter().all(|i| i.origin == Origin::User));
    }

    #[test]
    fn revisions_increase_on_every_change() {
        let (mut q, _) = queue_of(&["a", "b"]);
        let r0 = q.state().rev;
        q.go_next();
        q.set_repeat(Repeat::All);
        assert_eq!(q.state().rev, r0 + 2);
    }

    #[test]
    fn restore_keeps_ids_and_does_not_load() {
        let (q, _) = queue_of(&["a", "b", "c"]);
        let saved = q.state();
        let mut restored = Queue::new(7);
        restored.restore(saved.items.clone(), Some(saved.items[1].id.clone()), Repeat::All, true);
        let state = restored.state();
        assert_eq!(state.items, saved.items);
        assert_eq!(state.current_id.as_deref(), Some(saved.items[1].id.as_str()));
        assert!(state.shuffle);
        assert_eq!(state.repeat, Repeat::All);
    }

    #[test]
    fn folder_fill_lists_siblings_in_natural_order() {
        let dir = std::env::temp_dir().join(format!("vihpluh-queue-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["Show E10.mkv", "Show E2.mkv", "Show E1.mkv", "notes.txt"] {
            std::fs::write(dir.join(name), b"").unwrap();
        }
        let opened = dir.join("Show E2.mkv").display().to_string();
        let (items, index) = folder_fill(&opened).unwrap();
        // Forward slashes (as a frontend may send them) find the same file.
        let slashed = opened.replace('\\', "/");
        assert_eq!(folder_fill(&slashed).map(|(_, i)| i), Some(1));
        let names: Vec<String> = items.iter().map(|i| title_for(&i.source)).collect();
        assert_eq!(names, vec!["Show E1.mkv", "Show E2.mkv", "Show E10.mkv"]);
        assert_eq!(index, 1);
        assert!(items.iter().all(|i| i.origin == Origin::Folder));
        assert!(folder_fill(&dir.join("missing.mkv").display().to_string()).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn titles() {
        assert_eq!(title_for("D:/Shows/S02E03.mkv"), "S02E03.mkv");
        assert_eq!(title_for("https://example.com/a.m3u8"), "https://example.com/a.m3u8");
    }
}
