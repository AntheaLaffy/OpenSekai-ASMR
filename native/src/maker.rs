//! Chart editing on the original MusicScoreMaker prefab and note sprites.
use crate::{
    ffi::AppEvent,
    minimap::Minimap,
    model::{Note, Score},
    storage::{self, Entry},
    timeline::Timeline,
    ui::Painting,
    unity::{self, Bundle, CANVAS, Component, Scene, inside, number, reference},
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

#[derive(Clone)]
struct Snapshot {
    score: Score,
    selection: HashSet<i32>,
}
pub struct Document {
    pub score: Score,
    pub selection: HashSet<i32>,
    saved: Score,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}
impl Document {
    pub fn new(score: Score) -> Self {
        Self {
            saved: score.clone(),
            score,
            selection: HashSet::new(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
    pub fn dirty(&self) -> bool {
        self.score != self.saved
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            score: self.score.clone(),
            selection: self.selection.clone(),
        }
    }
    pub fn change(
        &mut self,
        edit: impl FnOnce(&mut Score, &mut HashSet<i32>),
    ) -> Result<(), String> {
        let before = self.snapshot();
        edit(&mut self.score, &mut self.selection);
        let issues = self.score.validate();
        if !issues.is_empty() {
            self.score = before.score;
            self.selection = before.selection;
            return Err(issues.join("; "));
        }
        if self.score != before.score {
            self.score.full_combo_data_hash = None;
            self.undo.push(before);
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        Ok(())
    }
    pub fn undo(&mut self) {
        if let Some(snapshot) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.score = snapshot.score;
            self.selection = snapshot.selection;
        }
    }
    pub fn redo(&mut self) {
        if let Some(snapshot) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.score = snapshot.score;
            self.selection = snapshot.selection;
        }
    }
    pub fn delete_selected(&mut self) -> Result<(), String> {
        self.change(|score, selected| {
            // Preserve reciprocal links when removing an interior connection.
            let links: HashMap<_, _> = score
                .note_list
                .iter()
                .map(|n| (n.id, (n.previous_connection_id, n.next_connection_id)))
                .collect();
            // The original presenter removes the whole chain when either end
            // is deleted; deleting interior points only reconnects neighbours.
            let mut pending: Vec<_> = score
                .note_list
                .iter()
                .filter(|n| {
                    selected.contains(&n.id)
                        && ((n.previous_connection_id == -1) != (n.next_connection_id == -1))
                })
                .map(|n| n.id)
                .collect();
            let mut visited = HashSet::new();
            while let Some(id) = pending.pop() {
                if !visited.insert(id) {
                    continue;
                }
                if let Some(&(prev, next)) = links.get(&id) {
                    selected.insert(id);
                    if prev >= 0 {
                        pending.push(prev);
                    }
                    if next >= 0 {
                        pending.push(next);
                    }
                }
            }
            for note in &mut score.note_list {
                for (connection, forward) in [
                    (&mut note.previous_connection_id, false),
                    (&mut note.next_connection_id, true),
                ] {
                    let mut visited = HashSet::new();
                    while selected.contains(connection) && visited.insert(*connection) {
                        *connection = links
                            .get(connection)
                            .map(|&(prev, next)| if forward { next } else { prev })
                            .unwrap_or(-1);
                    }
                }
            }
            score.note_list.retain(|n| !selected.contains(&n.id));
            selected.clear();
        })
    }
    pub fn save(&mut self, entry: &Entry) -> Result<(), String> {
        let issues = self.score.validate();
        if !issues.is_empty() {
            return Err(issues.join("; "));
        }
        storage::atomic_write(
            &entry.asset(&entry.manifest.score_file_name),
            self.score.to_json().map_err(|e| e.to_string())?.as_bytes(),
        )?;
        self.saved = self.score.clone();
        Ok(())
    }
    pub fn place(&mut self, mut note: Note, max_ticks: i64) -> Result<(), String> {
        note.id = self
            .score
            .note_list
            .iter()
            .map(|n| n.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Note IDs exhausted")?;
        note.ticks = note.ticks.clamp(0, max_ticks.max(0));
        let mut created = Vec::new();
        if matches!(note.category, 1 | 6 | 7 | 9) {
            note.ticks = note.ticks.min((max_ticks - 240).max(0));
            let mut end = note.clone();
            end.id = note.id.checked_add(1).ok_or("Note IDs exhausted")?;
            end.ticks = (note.ticks + 240).min(max_ticks);
            (end.category, end.note_base_type) = match note.category {
                1 => (1, 1),
                6 => (4, 11),
                7 => (5, 12),
                _ => (10, 13),
            };
            note.next_connection_id = end.id;
            end.previous_connection_id = note.id;
            created.push(note);
            created.push(end);
        } else {
            created.push(note);
        }
        self.change(|score, selection| {
            selection.clear();
            selection.extend(created.iter().map(|n| n.id));
            score.note_list.extend(created);
        })
    }
}

#[derive(Clone)]
enum Action {
    Event(String),
    Palette(Note),
    Quantize,
    Back,
    SelectMode,
    Lock,
    Critical,
    Play,
}
#[derive(Clone)]
struct Button {
    rect: [f32; 4],
    action: Action,
}
struct Gesture {
    point: [f32; 2],
    lane: i32,
    ticks: i64,
    note: Option<i32>,
    moved: bool,
}
pub struct Maker {
    pub scene: Scene,
    pub document: Document,
    pub entry: Entry,
    pub focus: i64,
    pub scale: f32,
    pub division: i32,
    pub status: String,
    pub exit: bool,
    tool: Option<Note>,
    buttons: Vec<Button>,
    gesture: Option<Gesture>,
    pressed_button: Option<usize>,
    menu: bool,
    locked: bool,
    palette_scroll: f32,
    english: bool,
    note_images: Value,
    connection_size: Option<[f32; 2]>,
    minimap_drag: bool,
    critical_filter: bool,
}
impl Maker {
    pub fn new(project: &Path, entry: Entry) -> Result<Self, String> {
        let bundle = Bundle::load(project)?;
        let mut scene = Scene::new(bundle.clone(), &bundle.main)?;
        let note_scene = Scene::new(bundle.clone(), &bundle.note)?;
        let note_images = note_scene
            .component("NotePreview")
            .ok_or("NotePreview component missing")?
            .fields
            .clone();
        let connection_size = note_scene
            .by_reference(reference(&note_images["_connectionImage"]))
            .map(|i| {
                let size = &note_scene.nodes[i].transform["m_SizeDelta"];
                [number(size, "x", 0.), number(size, "y", 0.)]
            });
        let score = entry.load_score()?;
        for node in &mut scene.nodes {
            if node
                .component("SubWindowSlideAnimationController")
                .is_some()
                || node.component("InvalidPlacementMessageView").is_some()
                || node.component("SelectedObjectEditUIView").is_some()
                || node.component("CriticalFilterToggleView").is_some()
            {
                node.active = false;
            }
            if node.component("CustomRawImage").is_some() {
                node.active = false;
            }
        }
        if let Some(c) = scene.component("MusicScoreMinimapView").cloned() {
            scene.set_reference_active(reference(&c.fields["_rawImage"]), true);
        }
        let palettes: Vec<_> = scene
            .nodes
            .iter()
            .filter_map(|n| n.component("SelectedNoteDataButton").cloned())
            .collect();
        for c in palettes {
            let note = note_from_button(&c);
            let icon = reference(&c.fields["_iconImage"]);
            scene.set_sprite_name(icon, &palette_icon(&note));
            if let Some(i) = scene.by_reference(icon) {
                scene.nodes[i].transform["m_LocalScale"]["x"] = json!(if !note.is_skip
                    && note.category == 3
                    && note.direction == 2
                {
                    -1.
                } else {
                    1.
                });
            }
        }
        if let Some(c) = scene.component("MusicPlayTimeView").cloned() {
            for key in [
                "_bpmEventSettingModeObject",
                "_highSpeedEventSettingModeObject",
                "_timeSignatureEventSettingModeObject",
                "_eventSettingModeButton",
            ] {
                scene.set_reference_active(reference(&c.fields[key]), false);
            }
            if let Some(i) = scene.by_reference(reference(&c.fields["_focusTickObj"])) {
                scene.nodes[i].transform["m_AnchorMin"]["y"] = json!(0.2);
                scene.nodes[i].transform["m_AnchorMax"]["y"] = json!(0.2);
                scene.nodes[i].transform["m_AnchoredPosition"]["y"] = json!(0.);
            }
        }
        if let Some(c) = scene.component("MusicControllerView").cloned() {
            scene.set_reference_active(reference(&c.fields["_stopImage"]), false);
        }
        if let Some(c) = scene.component("UIPartsToggle").cloned() {
            scene.set_reference_active(reference(&c.fields["desableGroup"]), false);
            scene.set_reference_active(reference(&c.fields["onGroup"]), false);
        }
        let mut maker = Self {
            scene,
            document: Document::new(score),
            entry,
            focus: 0,
            scale: 1.,
            division: 16,
            status: String::new(),
            exit: false,
            tool: None,
            buttons: Vec::new(),
            gesture: None,
            pressed_button: None,
            menu: false,
            locked: false,
            palette_scroll: 0.,
            english: false,
            note_images,
            connection_size,
            minimap_drag: false,
            critical_filter: false,
        };
        maker.update_scene();
        Ok(maker)
    }
    fn tr<'a>(&self, zh: &'a str, en: &'a str) -> &'a str {
        if self.english { en } else { zh }
    }
    pub fn notes_rect(&self) -> [f32; 4] {
        self.scene
            .find("NotesView")
            .map(|i| self.scene.nodes[i].rect)
            .unwrap_or([0.; 4])
    }
    pub fn max_ticks(&self) -> i64 {
        let seconds = self.entry.manifest.sec_for_music_score_maker;
        if seconds > 0 {
            Timeline::from_events(&self.document.score.music_score_event_data_list)
                .map(|t| t.ticks_at(seconds as f32))
                .unwrap_or(0)
                .max(0)
        } else {
            self.document.score.music_score_ticks_max.max(0)
        }
    }
    pub fn minimap_rect(&self) -> Option<[f32; 4]> {
        let c = self.scene.component("MusicScoreMinimapView")?;
        Some(self.scene.nodes[self.scene.by_reference(reference(&c.fields["_rawImage"]))?].rect)
    }
    fn range(&self) -> f32 {
        self.scale * 1920.
    }
    fn start_ticks(&self) -> f32 {
        self.focus as f32 - self.range() * 0.2
    }
    pub fn tick_y(&self, ticks: i64) -> f32 {
        let r = self.notes_rect();
        r[1] + r[3] * (1. - (ticks as f32 - self.start_ticks()) / self.range())
    }
    fn tick_at(&self, y: f32) -> i64 {
        let r = self.notes_rect();
        let raw = ((1. - (y - r[1]) / r[3]) * self.range()) as i64 + self.start_ticks() as i64;
        quantize(raw, 1920 / self.division).max(0)
    }
    fn lane_at(&self, x: f32) -> i32 {
        let r = self.notes_rect();
        (((x - r[0]) / r[2] * 12.).floor() as i32).clamp(0, 11)
    }
    fn update_scene(&mut self) {
        let controls: Vec<_> = self
            .scene
            .nodes
            .iter()
            .flat_map(|n| &n.components)
            .cloned()
            .collect();
        for c in controls {
            match c.kind.as_str() {
                "SelectedNoteDataButton" => {
                    let selected = self.tool.as_ref().is_some_and(|n| {
                        n.category == number(&c.fields, "_noteCategory", 0.) as i32
                            && n.note_type == number(&c.fields, "_noteTypes", 0.) as i32
                            && n.direction == number(&c.fields, "_noteDirection", 0.) as i32
                            && n.is_skip == (number(&c.fields, "_isSkip", 0.) != 0.)
                    });
                    self.scene
                        .set_reference_active(reference(&c.fields["_selectedIndicator"]), selected);
                }
                "ToolButton" => self
                    .scene
                    .set_reference_active(reference(&c.fields["_selectedIndicator"]), false),
                "MusicControllerView" => {
                    if let Ok(timeline) =
                        Timeline::from_events(&self.document.score.music_score_event_data_list)
                    {
                        let ms = (timeline.time_at(self.focus) * 1000.).round().max(0.) as i64;
                        self.scene.set_text(
                            reference(&c.fields["timeText"]),
                            &format!("{:02}:{:02}:{:03}", ms / 60000, ms / 1000 % 60, ms % 1000),
                        );
                    }
                }
                "CustomDropdown" => self.scene.set_text(
                    reference(&c.fields["m_CaptionText"]),
                    &format!("1/{}", self.division),
                ),
                "CustomInputFieldTextMesh" => self.scene.set_text(
                    reference(&c.fields["m_TextComponent"]),
                    &format!("{:.0}%", self.scale * 100.),
                ),
                _ => (),
            }
        }
        if let Some(i) = self.scene.find("MenuSubWindowComponent") {
            self.scene.nodes[i].active = self.menu;
        }
        self.scene.layout();
        if let Some(c) = self.scene.component("LaneLinePreview").cloned() {
            let width = self.notes_rect()[2];
            if let Some(lines) = c.fields["_laneLines"].as_array() {
                for (n, line) in lines.iter().enumerate() {
                    if let Some(i) = self.scene.by_reference(reference(line)) {
                        self.scene.nodes[i].transform["m_AnchoredPosition"]["x"] =
                            json!(width * (n as f32 - 6.) / 12.);
                    }
                }
            }
        }
        self.scene.layout();
        self.buttons.clear();
        for node in &self.scene.nodes {
            if !node.visible {
                continue;
            }
            let mut action = match node.name.as_str() {
                "BackButton" => Some(Action::Back),
                "QuantizeSettingsView" => Some(Action::Quantize),
                "UIPartsToggle" => Some(Action::SelectMode),
                "EditRestrictedToggleButton" => Some(Action::Lock),
                "CriticalFilterToggleView" => Some(Action::Critical),
                _ => None,
            };
            for c in &node.components {
                match c.kind.as_str() {
                    "DispatcherEventBaseButton" => {
                        action = c.fields["_eventClassName"]
                            .as_str()
                            .map(|s| Action::Event(s.into()))
                    }
                    "SelectedNoteDataButton" => action = Some(Action::Palette(note_from_button(c))),
                    "ToolButton" => {
                        action = Some(Action::Event(
                            if number(&c.fields, "_toolType", 0.) == 15. {
                                "UndoEvent"
                            } else {
                                "RedoEvent"
                            }
                            .into(),
                        ))
                    }
                    "MusicControllerView" => {
                        if let Some(i) =
                            self.scene.by_reference(reference(&c.fields["_playButton"]))
                        {
                            self.buttons.push(Button {
                                rect: self.scene.nodes[i].rect,
                                action: Action::Play,
                            });
                        }
                    }
                    _ => (),
                }
            }
            if let Some(action) = action {
                self.buttons.push(Button {
                    rect: unity::intersect(node.rect, node.clip),
                    action,
                });
            }
        }
    }
    pub fn draw(&mut self, paint: &mut Painting, english: bool) {
        self.english = english;
        self.update_scene();
        paint.rect(CANVAS, 0x444466ff, CANVAS);
        self.scene.draw_with(paint, english, &mut |node, paint| {
            if node.name == "BarLineView" {
                self.draw_grid(paint);
            }
            if node.name == "NotesView" {
                self.draw_long_notes(paint);
                self.draw_notes(paint);
            }
            if self
                .scene
                .component("MusicScoreMinimapView")
                .and_then(|c| self.scene.by_reference(reference(&c.fields["_rawImage"])))
                .is_some_and(|i| self.scene.nodes[i].id == node.id)
            {
                self.draw_minimap(paint, node.rect, node.clip);
            }
        });
        if !self.status.is_empty() {
            paint.rect([480., 14., 820., 42.], 0x15191fe8, CANVAS);
            paint.text(
                &self.status,
                [490., 18., 800., 32.],
                22.,
                false,
                1,
                0xffffffff,
                CANVAS,
            );
        }
    }
    fn draw_minimap(&self, paint: &mut Painting, rect: [f32; 4], clip: [f32; 4]) {
        let map = Minimap::new(&self.document.score, self.focus, self.max_ticks());
        map.draw(paint, rect, clip);
        let notes = self.notes_rect();
        let bottom = (notes[1] + notes[3] - CANVAS[3]).max(0.);
        let top = (-notes[1]).max(0.);
        let range = self.range() as i64;
        let start = self.start_ticks() as i64;
        let v0 = ((start + (bottom * range as f32 / notes[3]) as i64 - map.start) as f32
            / map.range as f32)
            .max(0.);
        let v1 = ((start + range - (top * range as f32 / notes[3]) as i64 - map.start) as f32
            / map.range as f32)
            .min(1.);
        if v1 <= v0 {
            return;
        }
        if let Some(guid) = self
            .scene
            .component("MusicScoreMinimapView")
            .and_then(|c| c.fields["_viewportFrameSprite"]["guid"].as_str())
        {
            self.scene.bundle.sprite(
                paint,
                guid,
                [
                    rect[0] - 6.,
                    rect[1] + (1. - v1) * rect[3] - 6.,
                    rect[2] + 12.,
                    (v1 - v0) * rect[3] + 12.,
                ],
                clip,
                0xffffffd9,
                true,
                false,
                1.,
                0.,
                false,
            );
        }
    }
    fn seek_minimap(&mut self, y: f32) {
        if let Some(rect) = self.minimap_rect() {
            let map = Minimap::new(&self.document.score, self.focus, self.max_ticks());
            self.focus = map.focus_at(rect, y, self.max_ticks());
        }
    }
    fn draw_grid(&self, paint: &mut Painting) {
        let r = self.notes_rect();
        let clip = unity::intersect(CANVAS, [r[0] - 140., r[1], r[2] + 140., r[3]]);
        let step = (1920 / self.division).max(1) as i64;
        let from = ((self.start_ticks() as i64).div_euclid(step) * step).max(0);
        let to = (self.start_ticks() + self.range()) as i64;
        for ticks in (from..=to).step_by(step as usize).take(4096) {
            let bar = ticks % 1920 == 0;
            let beat = ticks % 480 == 0;
            let y = self.tick_y(ticks);
            paint.rect(
                [
                    r[0] - if bar { 22. } else { 0. },
                    y - 1.5,
                    r[2] + if bar { 22. } else { 0. },
                    3.,
                ],
                if bar || beat { 0xffffffff } else { 0xa7a7bcff },
                clip,
            );
            if bar {
                paint.text(
                    &(ticks / 1920 + 1).to_string(),
                    [r[0] - 128., y - 16., 80., 32.],
                    32.,
                    true,
                    2,
                    0xffffffff,
                    clip,
                );
            }
        }
    }
    fn note_sprite(&self, note: &Note) -> Option<&str> {
        self.note_images["_notePreviewImages"]
            .as_array()?
            .iter()
            .find(|v| {
                number(v, "_noteType", 0.) as i32 == note.note_type
                    && number(v, "_noteCategory", 0.) as i32 == note.category
            })
            .and_then(|v| v["sprite"]["guid"].as_str())
            .or_else(|| self.note_images["_noInGameSprite"]["guid"].as_str())
    }
    fn note_rect(&self, note: &Note) -> [f32; 4] {
        let r = self.notes_rect();
        let unit = r[2] / 12.;
        let height = self
            .note_sprite(note)
            .and_then(|g| self.scene.bundle.sprites.get(g))
            .map(|s| s.rect[3] / s.pixels_per_unit)
            .unwrap_or(32.)
            * note_y_scale(self.scale);
        [
            r[0] + unit * note.lane_start as f32,
            self.tick_y(note.ticks) - height * 0.5,
            unit * (note.lane_end - note.lane_start + 1) as f32,
            height,
        ]
    }
    fn draw_long_notes(&self, paint: &mut Painting) {
        let Some(types) = self
            .scene
            .component("LongNoteLinesPreview")
            .and_then(|c| c.fields["_spriteTypes"].as_array())
        else {
            return;
        };
        let notes = &self.document.score.note_list;
        let ids: HashMap<_, _> = notes.iter().map(|n| (n.id, n)).collect();
        let rect = self.notes_rect();
        let clip = unity::intersect(CANVAS, rect);
        for root in notes
            .iter()
            .filter(|n| !n.is_skip && n.previous_connection_id == -1 && n.next_connection_id != -1)
        {
            let kind = if matches!(root.category, 1 | 6 | 7) {
                root.note_type
            } else if matches!(root.category, 9..=11) {
                2 + root.note_type
            } else {
                continue;
            };
            let Some(guid) = types
                .iter()
                .find(|s| number(s, "type", -1.) as i32 == kind)
                .and_then(|s| s["sprite"]["guid"].as_str())
            else {
                continue;
            };
            let Some(uv) = self.scene.bundle.sprite_uv(guid) else {
                continue;
            };
            let mut chain = vec![root];
            let mut current = root;
            let mut seen = HashSet::from([root.id]);
            while let Some(&next) = ids.get(&current.next_connection_id) {
                if !seen.insert(next.id) {
                    break;
                }
                if !next.is_skip {
                    chain.push(next);
                }
                current = next;
            }
            let edge = |n: &Note| {
                [
                    rect[0] + rect[2] / 12. * n.lane_start as f32,
                    self.tick_y(n.ticks),
                    rect[0] + rect[2] / 12. * (n.lane_end + 1) as f32,
                    self.tick_y(n.ticks),
                ]
            };
            for (index, pair) in chain.windows(2).enumerate() {
                let (a, b) = (pair[0], pair[1]);
                let selected = self.document.selection.contains(&a.id)
                    || self.document.selection.contains(&b.id);
                for quad in crate::long_notes::mesh(
                    edge(a),
                    edge(b),
                    a.note_line_type,
                    index,
                    chain.len() - 1,
                    uv,
                    clip,
                ) {
                    self.scene.bundle.quad(paint, guid, &quad, clip, selected);
                }
            }
        }
    }
    fn draw_notes(&self, paint: &mut Painting) {
        let clip = unity::intersect(CANVAS, self.notes_rect());
        for note in &self.document.score.note_list {
            if let Some(guid) = self.note_sprite(note) {
                let r = self.note_rect(note);
                if r[1] + r[3] < clip[1] || r[1] > clip[1] + clip[3] {
                    continue;
                }
                let start = paint.draws.len();
                if !note.is_skip {
                    self.scene
                        .bundle
                        .sprite(paint, guid, r, clip, 0xffffffff, true, false, 1., 0., false);
                }
                if self.document.selection.contains(&note.id) {
                    for draw in &mut paint.draws[start..] {
                        draw.reserved |= 2;
                    }
                }
                if matches!(note.category, 3 | 8)
                    && let Some(arrow) = self.note_images["_arrowPreviewImages"]
                        .as_array()
                        .and_then(|items| {
                            items.iter().find(|v| {
                                number(v, "_noteType", 0.) as i32 == note.note_type
                                    && number(v, "_noteDirection", 0.) as i32 == note.direction
                            })
                        })
                        .and_then(|v| v["sprite"]["guid"].as_str())
                {
                    let start = paint.draws.len();
                    let ys = note_y_scale(self.scale);
                    self.scene.bundle.sprite(
                        paint,
                        arrow,
                        [r[0], self.tick_y(note.ticks) - 134. * ys, r[2], 112. * ys],
                        clip,
                        0xffffffff,
                        true,
                        false,
                        1.,
                        0.,
                        false,
                    );
                    if note.direction != 1 {
                        for draw in &mut paint.draws[start..] {
                            draw.uv.swap(0, 2);
                        }
                    }
                    if self.document.selection.contains(&note.id) {
                        for draw in &mut paint.draws[start..] {
                            draw.reserved |= 2;
                        }
                    }
                }
                if let Some(size) = self.connection_size
                    && let Some(guid) = self.note_images["_connectionPreviewImages"]
                        .as_array()
                        .and_then(|items| {
                            items.iter().find(|v| {
                                number(v, "_noteType", 0.) as i32 == note.note_type
                                    && number(v, "_noteCategory", 0.) as i32 == note.category
                            })
                        })
                        .and_then(|v| v["sprite"]["guid"].as_str())
                {
                    let height = size[1] * note_y_scale(self.scale);
                    let start = paint.draws.len();
                    self.scene.bundle.sprite(
                        paint,
                        guid,
                        [
                            r[0] + (r[2] - size[0]) * 0.5,
                            self.tick_y(note.ticks) - height * 0.5,
                            size[0],
                            height,
                        ],
                        clip,
                        0xffffffff,
                        false,
                        false,
                        1.,
                        0.,
                        false,
                    );
                    if self.document.selection.contains(&note.id) {
                        for draw in &mut paint.draws[start..] {
                            draw.reserved |= 2;
                        }
                    }
                }
            }
        }
    }
    fn activate(&mut self, action: Action) -> Result<(), String> {
        match action {
            Action::Back => {
                if self.document.dirty() {
                    self.status = self
                        .tr(
                            "有未保存修改，请先保存（Ctrl+S）。",
                            "Save changes first (Ctrl+S).",
                        )
                        .into();
                } else {
                    self.exit = true;
                }
            }
            Action::Palette(note) => self.tool = Some(note),
            Action::Quantize => {
                let divisions = [4, 8, 12, 16, 24, 32, 48, 64];
                let i = divisions
                    .iter()
                    .position(|&d| d == self.division)
                    .unwrap_or(3);
                self.division = divisions[(i + 1) % divisions.len()];
            }
            Action::SelectMode => self.tool = None,
            Action::Lock => self.locked = !self.locked,
            Action::Critical => {
                self.critical_filter = !self.critical_filter;
                for node in &mut self.scene.nodes {
                    if let Some(c) = node.component("SelectedNoteDataButton")
                        && number(&c.fields, "_buttonType", 0.) == 1.
                    {
                        node.active =
                            (number(&c.fields, "_noteTypes", 0.) != 0.) == self.critical_filter;
                    }
                }
            }
            Action::Play => {
                return Err(self
                    .tr(
                        "音频预览仍在迁移中。",
                        "Audio preview is still being ported.",
                    )
                    .into());
            }
            Action::Event(event) => match event.as_str() {
                "QuickSaveMusicScoreEvent" | "SaveDraftScreenMusicScoreEvent" => {
                    self.document.save(&self.entry)?;
                    self.status = self.tr("谱面已保存。", "Chart saved.").into();
                }
                "UndoEvent" => self.document.undo(),
                "RedoEvent" => self.document.redo(),
                "RemoveNoteEvent" => self.document.delete_selected()?,
                "ZoomInTimelineEvent" => self.scale = (self.scale - 0.104).max(0.01),
                "ZoomOutTimelineEvent" => self.scale = (self.scale + 0.104).min(10.),
                "AddMusicScoreTimeSliderValue" => self.focus += (self.range() * 0.025) as i64,
                "SubtractMusicScoreTimeSliderValue" => {
                    self.focus = (self.focus - (self.range() * 0.025) as i64).max(0)
                }
                "ShowRightSubWindowEvent" => self.menu = !self.menu,
                "FlipSelectedNotesHorizontallyEvent" => {
                    self.document.change(|score, selected| {
                        for n in &mut score.note_list {
                            if selected.contains(&n.id) {
                                (n.lane_start, n.lane_end) = (11 - n.lane_end, 11 - n.lane_start);
                                if n.direction != 0 {
                                    n.direction = 3 - n.direction;
                                }
                            }
                        }
                    })?
                }
                _ => {
                    return Err(self
                        .tr("此功能仍在迁移中。", "This feature is still being ported.")
                        .into());
                }
            },
        }
        Ok(())
    }
    pub fn event(&mut self, e: &AppEvent) -> bool {
        let result = self.handle(e);
        if let Err(message) = result {
            self.status = message;
        }
        self.focus = self.focus.clamp(0, self.max_ticks());
        true
    }
    fn handle(&mut self, e: &AppEvent) -> Result<(), String> {
        match e.kind {
            1 if e.key == 1 => {
                if !self.menu && self.minimap_rect().is_some_and(|r| inside(r, e.x, e.y)) {
                    self.minimap_drag = true;
                    self.seek_minimap(e.y);
                    return Ok(());
                }
                self.pressed_button = self.buttons.iter().rposition(|b| inside(b.rect, e.x, e.y));
                if self.pressed_button.is_none()
                    && !self.menu
                    && inside(self.notes_rect(), e.x, e.y)
                {
                    let note = self
                        .document
                        .score
                        .note_list
                        .iter()
                        .rev()
                        .find(|n| inside(self.note_rect(n), e.x, e.y))
                        .map(|n| n.id);
                    if e.modifiers == 0 {
                        self.document.selection.clear();
                    }
                    if let Some(id) = note {
                        self.document.selection.insert(id);
                    }
                    self.gesture = Some(Gesture {
                        point: [e.x, e.y],
                        lane: self.lane_at(e.x),
                        ticks: self.tick_at(e.y),
                        note,
                        moved: false,
                    });
                }
            }
            3 => {
                if self.minimap_drag {
                    self.seek_minimap(e.y);
                } else if let Some(g) = &mut self.gesture {
                    g.moved |= (e.x - g.point[0]).abs() + (e.y - g.point[1]).abs() > 5.;
                }
            }
            2 if e.key == 1 => {
                if self.minimap_drag {
                    self.minimap_drag = false;
                    return Ok(());
                }
                if let Some(i) = self.pressed_button.take() {
                    if inside(self.buttons[i].rect, e.x, e.y) {
                        self.activate(self.buttons[i].action.clone())?;
                    }
                } else if let Some(g) = self.gesture.take() {
                    if self.locked {
                        return Ok(());
                    }
                    if g.note.is_some() && g.moved {
                        let dl = self.lane_at(e.x) - g.lane;
                        let dt = self.tick_at(e.y) - g.ticks;
                        self.document.change(|score, selection| {
                            for note in &mut score.note_list {
                                if selection.contains(&note.id) {
                                    note.lane_start += dl;
                                    note.lane_end += dl;
                                    note.ticks += dt;
                                }
                            }
                        })?;
                    } else if g.note.is_none()
                        && let Some(mut note) = self.tool.clone()
                    {
                        if !matches!(note.category, 0 | 1 | 3..=9) {
                            return Err(self
                                .tr(
                                    "长条连接编辑仍在迁移中。",
                                    "Connected-note editing is still being ported.",
                                )
                                .into());
                        }
                        note.ticks = g.ticks.min(self.max_ticks());
                        (note.lane_start, note.lane_end) = if g.moved {
                            (g.lane.min(self.lane_at(e.x)), g.lane.max(self.lane_at(e.x)))
                        } else {
                            lane_range(g.lane, 2)
                        };
                        self.document.place(note, self.max_ticks())?;
                    }
                }
            }
            4 => {
                if e.x < 330. {
                    self.palette_scroll = (self.palette_scroll - e.delta * 48.).clamp(0., 1800.);
                    if let Some(c) = self.scene.component("UIPartsLeftTabList").cloned()
                        && let Some(i) = self.scene.by_reference(reference(&c.fields["scrollRect"]))
                        && let Some(scroll) = self.scene.nodes[i].component("CustomScrollRect")
                        && let Some(content) = self
                            .scene
                            .by_reference(reference(&scroll.fields["m_Content"]))
                    {
                        self.scene.nodes[content].transform["m_AnchoredPosition"]["y"] =
                            json!(self.palette_scroll);
                    }
                } else {
                    self.focus = (self.focus + (e.delta * self.range() * 0.025) as i64).max(0);
                }
            }
            5 => match e.key {
                27 => {
                    if self.menu {
                        self.menu = false;
                    } else {
                        self.activate(Action::Back)?;
                    }
                }
                115 if e.modifiers != 0 => {
                    self.activate(Action::Event("QuickSaveMusicScoreEvent".into()))?
                }
                122 if e.modifiers != 0 => self.document.undo(),
                121 if e.modifiers != 0 => self.document.redo(),
                97 if e.modifiers != 0 => {
                    self.document.selection =
                        self.document.score.note_list.iter().map(|n| n.id).collect()
                }
                127 | 8 if !self.locked => self.document.delete_selected()?,
                _ => (),
            },
            _ => (),
        }
        Ok(())
    }
}
fn note_from_button(c: &Component) -> Note {
    let category = number(&c.fields, "_noteCategory", 0.) as i32;
    Note {
        category,
        note_base_type: note_base_type(category),
        note_type: number(&c.fields, "_noteTypes", 0.) as i32,
        direction: number(&c.fields, "_noteDirection", 0.) as i32,
        note_line_type: number(&c.fields, "_noteLineType", 0.) as i32,
        is_skip: number(&c.fields, "_isSkip", 0.) != 0.,
        ..Note::default()
    }
}
pub fn note_base_type(category: i32) -> i32 {
    match category {
        1 => 2,
        2 => 5,
        3 => 3,
        4 => 11,
        5 => 12,
        6 => 8,
        7 => 9,
        8 => 4,
        9 => 10,
        10 => 13,
        11 => 14,
        12 => 7,
        13 => 6,
        _ => 1,
    }
}
pub fn note_y_scale(scale: f32) -> f32 {
    if scale < 2. {
        1.
    } else if scale >= 4. {
        0.3
    } else {
        1. + (0.3 - 1.) * ((scale - 2.) / 2.).clamp(0., 1.)
    }
}
fn palette_icon(note: &Note) -> String {
    let critical = if note.note_type == 1 { "_crtc" } else { "" };
    let line = match note.note_line_type {
        1 => "_line_easeout",
        2 => "_line_easein",
        _ => "_line_linear",
    };
    let direction = match note.direction {
        1 => "_l",
        2 => "_r",
        _ => "",
    };
    let name = if note.is_skip {
        "notes_icon_long_among".to_owned()
    } else {
        match note.category {
            1 => "notes_icon_long".into(),
            2 => format!("notes_icon_long_among{line}"),
            3 => if note.direction == 0 {
                "notes_icon_frick"
            } else {
                "notes_icon_frick_l"
            }
            .into(),
            4 | 5 => "notes_icon_trace".into(),
            6 => "notes_icon_tracelong_line_linear".into(),
            7 => "notes_icon_clear".into(),
            8 => format!("notes_icon_tracefrick{direction}"),
            9 | 10 => "notes_icon_guide".into(),
            11 => format!("notes_icon_guide_among{line}"),
            12 | 14 => "notes_icon_long_among".into(),
            13 => format!("notes_icon_clear_among{line}"),
            _ => "notes_icon_normal".into(),
        }
    };
    format!("{name}{critical}")
}
pub fn quantize(ticks: i64, step: i32) -> i64 {
    if step <= 0 {
        return ticks;
    }
    let bar = ticks / 1920 * 1920;
    bar + (((ticks - bar) as f64 / step as f64).round() as i64) * step as i64
}
pub fn lane_range(center_lane: i32, width: i32) -> (i32, i32) {
    let width = width.max(0);
    let mut start = (center_lane - (width >> 1)).max(0);
    let mut end = start + width;
    if end > 11 {
        end = 11;
        start = (end - width).max(0);
    }
    (start, end)
}
