use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectSpec {
    pub schema_version: u32,
    pub project: ProjectInfo,
    pub milestones: Vec<Milestone>,
    pub layers: Vec<Layer>,
    pub modules: Vec<Module>,
    #[serde(default)]
    pub relations: Vec<Relation>,
    #[serde(default)]
    pub events: Vec<TimelineEvent>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectInfo {
    pub name: String,
    pub title: String,
    pub tagline: String,
    pub summary: String,
    pub status: String,
    pub repo: String,
    pub language: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Milestone {
    pub id: String,
    pub title: String,
    pub status: MilestoneStatus,
    pub goal: String,
    #[serde(default)]
    pub exit: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MilestoneStatus {
    Planned,
    Active,
    Done,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Layer {
    pub id: String,
    pub name: String,
    pub note: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Module {
    pub id: String,
    pub title: String,
    pub layer: String,
    pub status: ModuleStatus,
    pub summary: String,
    #[serde(default)]
    pub contract: String,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleStatus {
    Planned,
    Active,
    Done,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Relation {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TimelineEvent {
    pub date: String,
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub refs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CardFront {
    pub id: String,
    pub title: String,
    pub status: CardStatus,
    pub milestone: String,
    #[serde(default)]
    pub module: Option<String>,
    pub size: Size,
    #[serde(default)]
    pub priority: Option<String>,
    pub updated: String,
    #[serde(default)]
    pub completed: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub docs: Vec<String>,
    #[serde(default)]
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardStatus {
    Backlog,
    Ready,
    Doing,
    Review,
    Done,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Size {
    S,
    M,
    L,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DocFront {
    pub id: String,
    pub title: String,
    pub kind: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default = "default_order")]
    pub order: i64,
    #[serde(default)]
    pub summary: Option<String>,
}

fn default_order() -> i64 {
    999
}

#[derive(Debug, Clone, Serialize)]
pub struct CardOut {
    #[serde(flatten)]
    pub front: CardFront,
    pub body_html: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocOut {
    #[serde(flatten)]
    pub front: DocFront,
    pub body_html: String,
    pub path: String,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceFile {
    pub path: String,
    pub language: String,
    pub text: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleOut {
    #[serde(flatten)]
    pub module: Module,
    pub sources: Vec<SourceFile>,
}

#[derive(Debug, Serialize)]
pub struct Dashboard {
    pub project: ProjectInfo,
    pub milestones: Vec<Milestone>,
    pub layers: Vec<Layer>,
    pub modules: Vec<ModuleOut>,
    pub relations: Vec<Relation>,
    pub cards: Vec<CardOut>,
    pub docs: Vec<DocOut>,
    pub events: Vec<TimelineEvent>,
    pub meta: Meta,
}

#[derive(Debug, Serialize)]
pub struct Meta {
    pub schema_version: u32,
    pub cards: usize,
    pub modules: usize,
    pub docs: usize,
    pub generator: String,
}
