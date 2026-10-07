//! Native manager layout ported from ScreenLayerCustomMusicScoreManager.BuildView.
use crate::{
    ffi::{AppDraw, AppEvent, AppFrame},
    model::Manifest,
    packages,
    storage::{self, Entry},
};
use std::{ffi::CString, path::PathBuf};

pub const FIELDS: [(&str, &str, &str); 17] = [
    ("曲名", "Song title", "title"),
    ("谱面标题", "Chart title", "scoreTitle"),
    ("作者", "Author", "userName"),
    ("音频", "Audio", "audioFileName"),
    ("封面", "Jacket", "jacketFileName"),
    ("谱面", "Chart", "scoreFileName"),
    ("2DMV", "2DMV", "videoFileName"),
    ("前置空白秒", "Leading silence (s)", "fillerSec"),
    ("编辑时长秒", "Chart length (s)", "secForMusicScoreMaker"),
    ("难度", "Difficulty", "musicDifficultyType"),
    ("等级", "Level", "playLevel"),
    ("作曲", "Composer", "composer"),
    ("作词", "Lyricist", "lyricist"),
    ("编曲", "Arranger", "arranger"),
    ("歌手", "Singer", "singer"),
    ("联动标签", "Collaboration", "collaborationLabel"),
    ("描述", "Description", "description"),
];
const TEXT: u32 = 0xeef3f7ff;
const VIEW: [f32; 4] = [0., 0., 1920., 1080.];
const DIFFICULTIES: [&str; 6] = ["easy", "normal", "hard", "expert", "master", "append"];
const SEARCH_FIELD: usize = 17;
#[derive(Clone)]
struct Hit {
    rect: [f32; 4],
    action: usize,
}
#[derive(Default)]
pub struct Painting {
    pub draws: Vec<AppDraw>,
    pub(crate) strings: Vec<CString>,
}
impl Painting {
    pub(crate) fn rect(&mut self, rect: [f32; 4], color: u32, clip: [f32; 4]) {
        self.draws.push(AppDraw {
            kind: 0,
            rgba: color,
            rect,
            clip,
            ..AppDraw::default()
        });
    }
    // Mirrors the source text constructor plus its resolved rectangle and clip.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn text(
        &mut self,
        text: &str,
        rect: [f32; 4],
        size: f32,
        bold: bool,
        align: u32,
        color: u32,
        clip: [f32; 4],
    ) {
        let text = CString::new(text.replace('\0', "")).unwrap();
        let pointer = text.as_ptr();
        self.strings.push(text);
        self.draws.push(AppDraw {
            kind: 1,
            rgba: color,
            font: if bold { 1 } else { 2 },
            align,
            rect,
            clip,
            font_size: size,
            text: pointer,
            reserved: 0,
            ..AppDraw::default()
        });
    }
}
pub struct App {
    project: PathBuf,
    include_official: bool,
    pub maker: Option<crate::maker::Maker>,
    pub live: Option<crate::live::Live>,
    pub fonts: [CString; 2],
    pub samples: CString,
    pub painting: Painting,
    pub root: PathBuf,
    pub entries: Vec<Entry>,
    pub selected: Option<usize>,
    pub status: String,
    form: Vec<String>,
    search: String,
    filtered: Vec<usize>,
    pub focus: Option<usize>,
    select_all: bool,
    composition: String,
    input_rect: [f32; 4],
    hits: Vec<Hit>,
    scroll: f32,
    list_scroll: f32,
    settings_scroll: f32,
    settings: bool,
    pub english: bool,
    pub note_speed: f32,
    mouse: [f32; 2],
    pressed: Option<usize>,
    pub pending_request: u32,
    file_request_kind: Option<u32>,
    file_target: Option<PathBuf>,
}
fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3]
}
fn intersect(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let x = a[0].max(b[0]);
    let y = a[1].max(b[1]);
    [
        x,
        y,
        (a[0] + a[2]).min(b[0] + b[2]) - x,
        (a[1] + a[3]).min(b[1] + b[3]) - y,
    ]
}
impl App {
    pub fn new(project: &str, data: &str) -> Result<Self, String> {
        let project = PathBuf::from(project)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let paths = ["FOT-RodinNTLGPro-EB.otf", "FOT-RodinNTLGPro-DB.otf"]
            .map(|f| project.join("Assets/Resources/font").join(f));
        for p in &paths {
            if !p.is_file() {
                return Err(format!("Original font missing: {}", p.display()));
            }
        }
        let root = if data.is_empty() {
            let base = std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
                .ok_or("No application data directory")?;
            base.join("opensekai-rs/CustomMusicScores")
        } else {
            PathBuf::from(data)
        };
        let note_speed = std::fs::read(root.join("native-play-settings.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| value["noteSpeed"].as_f64())
            .filter(|speed| speed.is_finite())
            .unwrap_or(6.)
            .clamp(1., 12.) as f32;
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../baseline/manager-source.json"))
                .map_err(|e| e.to_string())?;
        let mut samples = source["texts"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v["text"].as_str())
            .collect::<Vec<_>>()
            .join(" ");
        for f in FIELDS {
            samples.push_str(&format!(" {} {} {}", f.0, f.1, f.2));
        }
        samples.push_str(" Ojsk Community 1.6.16 设置 刷新 新建 导入 编辑 游玩 自动 复制 导出ZIP 删除 保存配置 计算时长 请选择谱面 未命名谱面 暂无本地谱面 本地谱面 最佳成绩 缺少音频 缺少谱面 缺少封面 就绪 曲名 路径 更新 ID 已创建谱面 已保存配置 数据错误 原生运行时 此功能仍在迁移中 取消 保存 英语 English Settings Refresh New Import Edit Play Auto Duplicate Export ZIP Delete Save configuration Calculate duration Select a chart No local charts Local charts Best result No play history No audio No chart No jacket Ready Generate video Click to start MASTER EASY NORMAL HARD EXPERT APPEND ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz 0123456789 .,:;/-_()[] □ 音乐音量 音效音量 音符流速 判定偏移 上隐挡板 背景亮度 长条线不透明度 Guide线不透明度 判定线不透明度");
        samples = samples
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        samples.push('…');
        let mut app = Self {
            project,
            include_official: data.is_empty(),
            maker: None,
            live: None,
            fonts: paths.map(|p| CString::new(p.to_string_lossy().as_bytes()).unwrap()),
            samples: CString::new(samples).unwrap(),
            painting: Painting::default(),
            root,
            entries: Vec::new(),
            selected: None,
            status: String::new(),
            form: vec![String::new(); 17],
            search: String::new(),
            filtered: Vec::new(),
            focus: None,
            select_all: false,
            composition: String::new(),
            input_rect: [0.; 4],
            hits: Vec::new(),
            scroll: 0.,
            list_scroll: 0.,
            settings_scroll: 0.,
            settings: false,
            english: false,
            note_speed,
            mouse: [-1.; 2],
            pressed: None,
            pending_request: 0,
            file_request_kind: None,
            file_target: None,
        };
        app.refresh()?;
        Ok(app)
    }
    fn tr<'a>(&self, zh: &'a str, en: &'a str) -> &'a str {
        if self.english { en } else { zh }
    }
    pub fn set_note_speed(&mut self, speed: f32) -> Result<(), String> {
        if !speed.is_finite() {
            return Err("Note speed must be finite".into());
        }
        self.note_speed = (speed.clamp(1., 12.) * 10.).round() / 10.;
        storage::atomic_write(
            &self.root.join("native-play-settings.json"),
            &serde_json::to_vec_pretty(
                &serde_json::json!({"schema":1,"noteSpeed":self.note_speed}),
            )
            .unwrap(),
        )
    }
    fn refresh(&mut self) -> Result<(), String> {
        let selected_id = self
            .selected
            .and_then(|i| self.entries.get(i))
            .map(|e| e.manifest.id.clone());
        let (mut entries, mut warnings) = storage::scan(&self.root)?;
        if self.include_official {
            let (mut official, problems) = storage::scan(&self.project.join("content/library"))?;
            official.sort_by_key(|e| {
                (
                    e.manifest
                        .extra
                        .get("sourceSus")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&e.manifest.id)
                        .split('/')
                        .next()
                        .unwrap_or("")
                        .to_owned(),
                    DIFFICULTIES
                        .iter()
                        .position(|d| *d == e.manifest.music_difficulty_type)
                        .unwrap_or(6),
                )
            });
            entries.extend(official);
            warnings.extend(problems);
        }
        self.entries = entries;
        let select = selected_id
            .and_then(|id| self.entries.iter().position(|e| e.manifest.id == id))
            .or({
                if self.entries.is_empty() {
                    None
                } else {
                    Some(0)
                }
            });
        self.select(select);
        if !warnings.is_empty() {
            self.status = warnings.join("; ");
        }
        Ok(())
    }
    fn select(&mut self, index: Option<usize>) {
        self.filter_entries();
        let index = index.filter(|&i| i < self.entries.len());
        self.selected = index;
        self.focus = None;
        self.composition.clear();
        self.scroll = 0.;
        let value = index.map(|i| serde_json::to_value(&self.entries[i].manifest).unwrap());
        self.form = FIELDS
            .iter()
            .map(|f| {
                value
                    .as_ref()
                    .map(|v| match &v[f.2] {
                        serde_json::Value::String(s) => s.clone(),
                        v => v.to_string(),
                    })
                    .unwrap_or_default()
            })
            .collect();
    }
    fn filter_entries(&mut self) {
        let query = self.search.to_lowercase();
        let terms: Vec<_> = query.split_whitespace().collect();
        self.filtered = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if terms.is_empty() {
                    return Some(i);
                }
                let m = &e.manifest;
                let text = format!(
                    "{} {} {} {} {} {} {}",
                    m.id,
                    m.title,
                    m.score_title,
                    m.user_name,
                    m.composer,
                    m.music_difficulty_type,
                    m.play_level
                )
                .to_lowercase();
                terms.iter().all(|term| text.contains(term)).then_some(i)
            })
            .collect();
    }
    fn search_changed(&mut self) {
        self.filter_entries();
        self.list_scroll = 0.;
        if self.selected.is_none_or(|i| !self.filtered.contains(&i)) {
            self.select(self.filtered.first().copied());
            self.focus = Some(SEARCH_FIELD);
        }
    }
    fn focused_value(&mut self, index: usize) -> &mut String {
        if index == SEARCH_FIELD {
            &mut self.search
        } else {
            &mut self.form[index]
        }
    }
    fn save(&mut self) -> Result<(), String> {
        let i = self.selected.ok_or("No chart selected")?;
        let mut value =
            serde_json::to_value(&self.entries[i].manifest).map_err(|e| e.to_string())?;
        for (n, field) in FIELDS.iter().enumerate() {
            value[field.2] = if matches!(n, 7 | 8 | 10) {
                let v: serde_json::Value = serde_json::from_str(&self.form[n])
                    .map_err(|_| format!("{}: invalid number", field.0))?;
                if !v.is_number() {
                    return Err(format!("{}: invalid number", field.0));
                }
                v
            } else {
                self.form[n].clone().into()
            };
        }
        let manifest: Manifest = serde_json::from_value(value).map_err(|e| e.to_string())?;
        let mut entry = self.entries[i].clone();
        entry.manifest = manifest;
        entry.save_manifest()?;
        self.entries[i] = entry;
        self.select(Some(i));
        self.status = self.tr("已保存配置。", "Configuration saved.").into();
        Ok(())
    }
    fn button(
        &mut self,
        label: &str,
        r: [f32; 4],
        action: usize,
        enabled: bool,
        danger: bool,
        clip: [f32; 4],
    ) {
        let base = if danger {
            [110u32, 49, 57, 255]
        } else {
            [54, 67, 80, 255]
        };
        let tint = if !enabled {
            [120, 126, 132, 120]
        } else if self.pressed == Some(action) {
            [180, 213, 242, 255]
        } else if inside(r, self.mouse[0], self.mouse[1]) {
            [220, 238, 255, 255]
        } else {
            [255; 4]
        };
        let c = (0..4).fold(0, |c, i| (c << 8) | ((base[i] * tint[i] + 127) / 255));
        self.painting.rect(r, c, clip);
        self.painting.text(label, r, 20., true, 1, TEXT, clip);
        if enabled {
            self.hits.push(Hit {
                rect: intersect(r, clip),
                action,
            });
        }
    }
    fn manager(&mut self) {
        self.painting.rect(VIEW, 0x15191fff, VIEW);
        self.painting.rect([0., 0., 1920., 108.], 0x1f252dff, VIEW);
        self.painting.text(
            "Ojsk Community 1.6.16",
            [36., 0., 560., 108.],
            40.,
            true,
            0,
            TEXT,
            VIEW,
        );
        let mut x = 1260.;
        for (id, zh, en, w) in [
            (1, "设置", "Settings", 150.),
            (2, "刷新", "Refresh", 150.),
            (3, "新建", "New", 132.),
            (4, "导入", "Import", 150.),
        ] {
            let label = self.tr(zh, en).to_owned();
            self.button(&label, [x, 26., w, 56.], id, true, false, VIEW);
            x += w + 14.;
        }
        self.painting
            .rect([56., 136., 620., 916.], 0x1a1f26ff, VIEW);
        self.painting.text(
            self.tr("本地谱面", "Local charts"),
            [74., 156., 174., 50.],
            26.,
            true,
            0,
            TEXT,
            VIEW,
        );
        let search_rect = [250., 156., 390., 50.];
        self.painting.rect(
            search_rect,
            if self.focus == Some(SEARCH_FIELD) {
                0x304353ff
            } else {
                0x161a20ff
            },
            VIEW,
        );
        let search_text = if self.focus == Some(SEARCH_FIELD) {
            self.input_rect = search_rect;
            format!("{}{}│", self.search, self.composition)
        } else if self.search.is_empty() {
            self.tr("搜索曲名 / ID / 难度", "Search title / ID / difficulty")
                .into()
        } else {
            self.search.clone()
        };
        self.painting.text(
            &search_text,
            [260., 156., 370., 50.],
            20.,
            false,
            0,
            TEXT,
            search_rect,
        );
        self.hits.push(Hit {
            rect: search_rect,
            action: 1017,
        });
        let list_clip = [72., 224., 588., 812.];
        if self.filtered.is_empty() {
            self.painting.text(
                if self.entries.is_empty() {
                    self.tr("暂无本地谱面", "No local charts")
                } else {
                    self.tr("没有匹配的谱面", "No matching charts")
                },
                [84., 236., 564., 716.],
                24.,
                false,
                1,
                TEXT,
                VIEW,
            );
        }
        for (row, &i) in self.filtered.iter().enumerate() {
            let y = 234. + row as f32 * 128. - self.list_scroll;
            let r = [82., y, 568., 116.];
            if y + 116. < 224. || y > 1036. {
                continue;
            }
            let entry = &self.entries[i];
            self.painting.rect(
                r,
                if self.selected == Some(i) {
                    0x345b70ff
                } else {
                    0x272e37ff
                },
                list_clip,
            );
            self.painting.text(
                &entry.manifest.score_title,
                [104., y + 14., 524., 36.],
                26.,
                true,
                0,
                TEXT,
                list_clip,
            );
            self.painting.text(
                &format!(
                    "Lv.{}  {}  {}",
                    entry.manifest.play_level,
                    entry.manifest.user_name,
                    entry.manifest.music_difficulty_type.to_uppercase()
                ),
                [104., y + 70., 358., 30.],
                20.,
                false,
                0,
                TEXT,
                list_clip,
            );
            let color = if entry.has_audio() && entry.has_score() {
                0x7edda6ff
            } else {
                0xffb864ff
            };
            self.painting.text(
                entry.status(),
                [450., y + 70., 178., 30.],
                20.,
                true,
                2,
                color,
                list_clip,
            );
            self.hits.push(Hit {
                rect: intersect(r, list_clip),
                action: 10000 + i,
            });
        }
        self.painting
            .rect([716., 136., 1176., 916.], 0x1c222aff, VIEW);
        self.painting
            .rect([744., 164., 200., 200.], 0x2a313aff, VIEW);
        let selected = self.selected.map(|i| self.entries[i].clone());
        let title = selected
            .as_ref()
            .map(|e| e.manifest.score_title.as_str())
            .unwrap_or(self.tr("请选择谱面", "Select a chart"));
        let right = if selected.is_some() { 475.36 } else { 30. };
        self.painting.text(
            title,
            [968., 166., 1176. - 252. - right, 54.],
            38.,
            true,
            0,
            TEXT,
            VIEW,
        );
        if let Some(e) = &selected {
            let meta = [
                format!("曲名：{}", e.manifest.title),
                format!("ID：{}", e.manifest.id),
                format!("路径：{}", e.directory.display()),
            ];
            for (i, line) in meta.iter().enumerate() {
                self.painting.text(
                    line,
                    [968., 224. + i as f32 * 22., 448.64, 22.],
                    22.,
                    false,
                    0,
                    TEXT,
                    VIEW,
                );
            }
            self.painting.text(
                e.status(),
                [968., 320., 448.64, 38.],
                22.,
                true,
                0,
                if e.has_audio() && e.has_score() {
                    0x7edda6ff
                } else {
                    0xffb864ff
                },
                VIEW,
            );
            let best_x = 1440.64;
            self.painting
                .rect([best_x, 164., 423.36, 168.], 0x161b22ff, VIEW);
            self.painting.text(
                self.tr("最佳成绩", "Best result"),
                [best_x + 18., 178., 387.36, 30.],
                22.,
                true,
                0,
                TEXT,
                VIEW,
            );
            self.painting.text(
                self.tr("暂无游玩记录", "No play history"),
                [best_x + 18., 214., 181.68, 92.],
                18.,
                false,
                0,
                TEXT,
                VIEW,
            );
        }
        let action_width = (1120. - 60.) / 6.;
        self.painting.text(
            self.tr("音符流速", "Note speed"),
            [1440., 340., 116., 44.],
            20.,
            true,
            0,
            TEXT,
            VIEW,
        );
        self.button(
            "-",
            [1560., 338., 60., 44.],
            40,
            self.note_speed > 1.,
            false,
            VIEW,
        );
        self.painting.text(
            &format!("{:.1}", self.note_speed),
            [1628., 339., 96., 44.],
            28.,
            true,
            1,
            0x8ee8ffff,
            VIEW,
        );
        self.button(
            "+",
            [1730., 338., 60., 44.],
            41,
            self.note_speed < 12.,
            false,
            VIEW,
        );
        let reset = self.tr("重置", "Reset").to_owned();
        self.button(&reset, [1798., 338., 66., 44.], 42, true, false, VIEW);
        for (n, zh, en) in [
            (0, "编辑", "Edit"),
            (1, "游玩", "Play"),
            (2, "自动", "Auto"),
            (3, "复制", "Duplicate"),
            (4, "导出ZIP", "Export ZIP"),
            (5, "删除", "Delete"),
        ] {
            let enabled = selected
                .as_ref()
                .is_some_and(|e| !matches!(n, 1 | 2) || e.has_score());
            let label = if n == 1 && selected.as_ref().is_some_and(|e| !e.has_audio()) {
                self.tr("无音乐练习", "Practice").to_owned()
            } else {
                self.tr(zh, en).to_owned()
            };
            self.button(
                &label,
                [
                    744. + n as f32 * (action_width + 12.),
                    392.,
                    action_width,
                    54.,
                ],
                10 + n,
                enabled,
                n == 5,
                VIEW,
            );
        }
        let form_clip = [744., 468., 1120., 472.];
        self.painting.rect(form_clip, 0x181d24ff, VIEW);
        let cell: f32 = (1056. - 44.) / 3.; // Floor below follows UpdateManifestFieldLayout.
        let cell = cell.floor();
        for (n, field) in FIELDS.iter().enumerate() {
            let x = 764. + (n % 3) as f32 * (cell + 22.);
            let y = 488. + (n / 3) as f32 * 136. - self.scroll;
            if y + 116. < 468. || y > 940. {
                continue;
            }
            self.painting.text(
                self.tr(field.0, field.1),
                [x, y, cell, 30.],
                23.,
                true,
                0,
                TEXT,
                form_clip,
            );
            let r = [x, y + 42., cell, 70.];
            self.painting.rect(r, 0x161a20ff, form_clip);
            let active = self.focus == Some(n);
            let value = if active {
                format!("{}{}", self.form[n], self.composition)
            } else {
                self.form[n].clone()
            };
            let placeholder = value.is_empty();
            let value = if n == 9 {
                if value.is_empty() {
                    "MASTER".into()
                } else {
                    value.to_uppercase()
                }
            } else {
                value
            };
            let import = matches!(n, 3..=6);
            let width = cell - if import { 148. } else { 36. };
            self.painting.text(
                if placeholder && n != 9 {
                    field.2
                } else {
                    &value
                },
                [x + 18., y + 49., width, 56.],
                26.,
                false,
                0,
                if placeholder { 0x88929eb4 } else { TEXT },
                form_clip,
            );
            if active {
                self.input_rect = r;
                self.painting
                    .rect([x, y + 110., cell, 2.], 0x84bde8ff, form_clip);
            }
            if selected.is_some() {
                self.hits.push(Hit {
                    rect: intersect(r, form_clip),
                    action: 1000 + n,
                });
            }
            if import {
                let label = self.tr("导入", "Import").to_owned();
                self.button(
                    &label,
                    [x + cell - 117., y + 49., 110., 56.],
                    30 + n,
                    selected.is_some(),
                    false,
                    form_clip,
                );
            }
        }
        let label = self.tr("保存配置", "Save configuration").to_owned();
        self.button(
            &label,
            [744., 963., 230., 58.],
            5,
            selected.is_some(),
            false,
            VIEW,
        );
        let label = self.tr("计算时长", "Calculate duration").to_owned();
        self.button(
            &label,
            [974., 963., 160., 58.],
            6,
            selected.as_ref().is_some_and(Entry::has_audio),
            false,
            VIEW,
        );
        self.painting.text(
            &self.status,
            [1134., 963., 730., 58.],
            21.,
            false,
            2,
            TEXT,
            VIEW,
        );
    }
    fn settings(&mut self) {
        self.hits.clear();
        self.painting.rect(VIEW, 0x000000b0, VIEW);
        self.painting
            .rect([580., 60., 760., 960.], 0x1f252dff, VIEW);
        self.painting.text(
            self.tr("设置", "Settings"),
            [612., 92., 696., 46.],
            36.,
            true,
            0,
            TEXT,
            VIEW,
        );
        let clip = [612., 154., 696., 760.];
        self.painting.rect(clip, 0x181d24ff, VIEW);
        for (i, zh, en, value) in [
            (0, "音乐音量", "Music volume", "1.0"),
            (1, "音效音量", "Sound volume", "1.0"),
            (2, "音符流速", "Note speed", "6.0"),
            (3, "判定偏移", "Timing adjustment", "0.0"),
            (4, "上隐挡板", "Hidden cover", "0"),
            (5, "背景亮度", "Brightness", "100"),
            (6, "长条线不透明度", "Hold opacity", "100"),
            (7, "Guide线不透明度", "Guide opacity", "100"),
            (8, "判定线不透明度", "Judge line opacity", "100"),
        ] {
            let y = 170. + i as f32 * 132. - self.settings_scroll;
            self.painting.text(
                self.tr(zh, en),
                [628., y, 664., 30.],
                23.,
                true,
                0,
                TEXT,
                clip,
            );
            self.painting
                .rect([628., y + 42., 664., 70.], 0x161a20ff, clip);
            let value = if i == 2 {
                format!("{:.1}", self.note_speed)
            } else {
                value.to_owned()
            };
            self.painting.text(
                &value,
                [646., y + 49., 628., 56.],
                26.,
                false,
                0,
                TEXT,
                clip,
            );
            if i == 2 {
                self.button(
                    "-",
                    [1090., y + 49., 80., 56.],
                    40,
                    self.note_speed > 1.,
                    false,
                    clip,
                );
                self.button(
                    "+",
                    [1180., y + 49., 80., 56.],
                    41,
                    self.note_speed < 12.,
                    false,
                    clip,
                );
            }
        }
        let label = self.tr("取消", "Cancel").to_owned();
        self.button(&label, [994., 933., 150., 52.], 20, true, false, VIEW);
        let label = self.tr("保存", "Save").to_owned();
        self.button(&label, [1158., 933., 150., 52.], 21, true, false, VIEW);
    }
    pub fn update(&mut self, dt: f32) {
        if let Some(live) = &mut self.live {
            live.update(dt);
        }
    }
    pub fn build(&mut self, english: bool) -> AppFrame {
        self.english = english;
        self.painting.draws.clear();
        self.painting.strings.clear();
        self.hits.clear();
        if let Some(live) = &self.live {
            live.draw(&mut self.painting, english);
        } else if let Some(maker) = &mut self.maker {
            maker.draw(&mut self.painting, english);
        } else {
            self.manager();
        }
        if self.settings {
            self.settings();
        }
        let mut frame = AppFrame {
            draws: self.painting.draws.as_ptr(),
            count: self.painting.draws.len() as u32,
            wants_text: u32::from(self.focus.is_some()),
            width: 1920.,
            height: 1080.,
            input_rect: self.input_rect,
            ..AppFrame::default()
        };
        if let Some(live) = &mut self.live {
            live.audio_frame(&mut frame);
            frame.reserved = u32::from(live.preparing) | (u32::from(live.result_ready()) << 1);
        }
        frame
    }
    fn activate(&mut self, action: usize) -> Result<(), String> {
        match action {
            1 => {
                self.settings = true;
                self.focus = None;
            }
            2 => self.refresh()?,
            3 => {
                let entry = storage::create(&self.root)?;
                self.entries.insert(0, entry);
                self.select(Some(0));
                self.status = self.tr("已创建谱面。", "Chart created.").into();
            }
            5 => self.save()?,
            10 => {
                let entry = self
                    .entries
                    .get(self.selected.ok_or("No chart selected")?)
                    .ok_or("No chart selected")?
                    .clone();
                self.maker = Some(crate::maker::Maker::new(&self.project, entry)?);
                self.focus = None;
            }
            11 | 12 => {
                let entry = self
                    .entries
                    .get(self.selected.ok_or("No chart selected")?)
                    .ok_or("No chart selected")?
                    .clone();
                let mut live = crate::live::Live::new(&self.project, entry, action == 12)?;
                live.note_speed = self.note_speed;
                self.live = Some(live);
                self.focus = None;
            }
            4 => self.request_file(1)?,
            13 => {
                let i = self.selected.ok_or("No chart selected")?;
                let copy = packages::duplicate(&self.entries[i], &self.root)?;
                self.entries.insert(0, copy);
                self.select(Some(0));
                self.status = self.tr("已复制谱面。", "Chart duplicated.").into();
            }
            14 => self.request_file(6)?,
            33..=36 => self.request_file((action - 31) as u32)?,
            20 => self.settings = false,
            21 => {
                self.set_note_speed(self.note_speed)?;
                self.settings = false;
            }
            40 => self.set_note_speed(self.note_speed - 0.1)?,
            41 => self.set_note_speed(self.note_speed + 0.1)?,
            42 => self.set_note_speed(6.)?,
            1017 => {
                self.focus = Some(SEARCH_FIELD);
                self.select_all = true;
                self.composition.clear();
            }
            10000.. => self.select(Some(action - 10000)),
            1000..=1016 => {
                let field = action - 1000;
                if field == 9 {
                    let i = DIFFICULTIES
                        .iter()
                        .position(|v| *v == self.form[9])
                        .unwrap_or(4);
                    self.form[9] = DIFFICULTIES[(i + 1) % 6].into();
                } else {
                    self.focus = Some(field);
                    self.select_all = true;
                    self.composition.clear();
                }
            }
            _ => {
                return Err(self
                    .tr("此功能仍在迁移中。", "This feature is still being ported.")
                    .into());
            }
        }
        Ok(())
    }
    pub fn event(&mut self, e: &AppEvent, text: &str) -> bool {
        if e.kind == 15 {
            self.update(e.delta);
            return true;
        }
        if let Some(live) = &mut self.live {
            let used = live.event(e);
            if live.exit {
                self.live = None;
            }
            return used;
        }
        if e.kind == 11 && e.key == 10 {
            if let Some(i) = self
                .entries
                .iter()
                .position(|entry| entry.manifest.id == text)
            {
                self.select(Some(i));
                return true;
            }
            return false;
        }
        if e.kind == 11 && matches!(e.key, 3 | 4) {
            if let Err(message) = self.activate(if e.key == 3 { 11 } else { 12 }) {
                eprintln!("OpenSekai live: {message}");
                self.status = message;
            }
            return self.live.is_some();
        }
        if e.kind == 11 && e.key == 2 {
            // Repeated host navigation must not replace a live edit session.
            if self.maker.is_some() {
                return true;
            }
            if let Err(message) = self.activate(10) {
                eprintln!("OpenSekai maker: {message}");
                self.status = message;
            }
            return self.maker.is_some();
        }
        if self.maker.is_some() && e.kind <= 8 {
            let maker = self.maker.as_mut().unwrap();
            maker.event(e);
            if maker.exit {
                self.maker = None;
            }
            return true;
        }
        match e.kind {
            1..=3 => {
                if e.kind != 3 && e.key != 1 {
                    return false;
                }
                self.mouse = [e.x, e.y];
                let action = self
                    .hits
                    .iter()
                    .rev()
                    .find(|h| inside(h.rect, e.x, e.y))
                    .map(|h| h.action);
                if e.kind == 1 {
                    self.pressed = action;
                    if action.is_none_or(|a| !(1000..=1017).contains(&a)) {
                        self.focus = None;
                        self.composition.clear();
                    }
                }
                if e.kind == 2 {
                    if action.is_some()
                        && action == self.pressed
                        && let Err(err) = self.activate(action.unwrap())
                    {
                        self.status = err;
                    }
                    self.pressed = None;
                }
                true
            }
            4 => {
                if self.settings {
                    self.settings_scroll = (self.settings_scroll - e.delta * 48.).clamp(0., 460.);
                } else if e.x < 688. {
                    self.list_scroll = (self.list_scroll - e.delta * 48.)
                        .clamp(0., (self.filtered.len() as f32 * 128. - 804.).max(0.));
                } else {
                    self.scroll = (self.scroll - e.delta * 48.).clamp(0., 360.);
                }
                true
            }
            5 => {
                if e.key == 102 && e.modifiers != 0 && !self.settings {
                    self.focus = Some(SEARCH_FIELD);
                    self.select_all = true;
                    return true;
                }
                if e.key == 27 {
                    self.settings = false;
                    self.focus = None;
                    self.composition.clear();
                    return true;
                }
                if let Some(n) = self.focus {
                    if e.key == 8 {
                        if self.select_all {
                            self.focused_value(n).clear();
                            self.select_all = false;
                        } else {
                            self.focused_value(n).pop();
                        }
                        if n == SEARCH_FIELD {
                            self.search_changed();
                        }
                    } else if e.key == 13 {
                        self.focus = None;
                        self.composition.clear();
                    } else if e.key == 97 && e.modifiers != 0 {
                        self.select_all = true;
                    }
                    return true;
                }
                false
            }
            6 => {
                if let Some(n) = self.focus {
                    if self.select_all {
                        self.focused_value(n).clear();
                        self.select_all = false;
                    }
                    self.focused_value(n).push_str(text);
                    self.composition.clear();
                    if n == SEARCH_FIELD {
                        self.search_changed();
                    }
                    true
                } else {
                    false
                }
            }
            7 if self.focus.is_some() => {
                self.composition = text.into();
                true
            }
            9 => {
                if self.file_request_kind != Some(e.key) {
                    return false;
                }
                if !text.is_empty() {
                    match self.file_selected(e.key, text) {
                        Ok(()) => {
                            self.status = self.tr("操作已完成。", "Operation completed.").into()
                        }
                        Err(e) => self.status = e,
                    }
                }
                self.file_request_kind = None;
                self.file_target = None;
                true
            }
            10 => {
                self.file_request_kind = None;
                self.file_target = None;
                self.status = format!("{}: {text}", self.tr("无法选择文件", "File dialog failed"));
                true
            }
            _ => false,
        }
    }
    fn request_file(&mut self, kind: u32) -> Result<(), String> {
        if self.file_request_kind.is_some() {
            return Err(self
                .tr(
                    "请先关闭文件选择窗口。",
                    "Close the current file dialog first.",
                )
                .into());
        }
        self.file_target = self.selected.map(|i| self.entries[i].directory.clone());
        self.pending_request = kind;
        self.file_request_kind = Some(kind);
        Ok(())
    }
    fn file_selected(&mut self, kind: u32, path: &str) -> Result<(), String> {
        let path = std::path::Path::new(path);
        if kind == 1 {
            let entry = if path.is_dir() {
                packages::import_folder(path, &self.root)?
            } else {
                packages::import_zip(path, &self.root)?
            };
            self.entries.insert(0, entry);
            self.select(Some(0));
            return Ok(());
        }
        let i = self
            .file_target
            .as_ref()
            .and_then(|path| self.entries.iter().position(|e| &e.directory == path))
            .ok_or("The selected chart no longer exists")?;
        if kind == 6 {
            return packages::export_zip(&self.entries[i], path);
        }
        let slot = match kind {
            2 => "audio",
            3 => "jacket",
            4 => "score",
            5 => "video",
            _ => return Err("Unknown file request".into()),
        };
        packages::replace_file(&mut self.entries[i], slot, path)?;
        self.select(Some(i));
        Ok(())
    }
}
