use std::collections::HashSet;
use std::path::Path;

use anyhow::{bail, Result};

use crate::model::{CardOut, CardStatus, DocOut, ModuleStatus, ProjectSpec};

pub fn validate(root: &Path, spec: &ProjectSpec, cards: &[CardOut], docs: &[DocOut]) -> Result<()> {
    let mut errors = Vec::new();

    if spec.schema_version != 1 {
        errors.push(format!(
            "unsupported schema_version {} (expected 1)",
            spec.schema_version
        ));
    }
    if spec.project.name.trim().is_empty() {
        errors.push("project.name is empty".into());
    }
    if spec.project.title.trim().is_empty() {
        errors.push("project.title is empty".into());
    }
    if spec.project.summary.trim().is_empty() {
        errors.push("project.summary is empty".into());
    }
    if spec.project.status.trim().is_empty() {
        errors.push("project.status is empty".into());
    }

    let mut milestone_ids = HashSet::new();
    for milestone in &spec.milestones {
        if !milestone_ids.insert(milestone.id.as_str()) {
            errors.push(format!("duplicate milestone id {}", milestone.id));
        }
        if milestone.title.trim().is_empty() {
            errors.push(format!("milestone {} has an empty title", milestone.id));
        }
    }
    if spec.milestones.is_empty() {
        errors.push("at least one milestone is required".into());
    }
    // The experience track (M9) runs alongside the remaining M6 review work.

    let mut layer_ids = HashSet::new();
    for layer in &spec.layers {
        if !layer_ids.insert(layer.id.as_str()) {
            errors.push(format!("duplicate layer id {}", layer.id));
        }
        if layer.name.trim().is_empty() {
            errors.push(format!("layer {} has an empty name", layer.id));
        }
    }
    if spec.layers.is_empty() {
        errors.push("at least one layer is required".into());
    }

    let mut module_ids = HashSet::new();
    for module in &spec.modules {
        if !module_ids.insert(module.id.as_str()) {
            errors.push(format!("duplicate module id {}", module.id));
        }
        if !layer_ids.contains(module.layer.as_str()) {
            errors.push(format!(
                "module {} references unknown layer {}",
                module.id, module.layer
            ));
        }
        if module.status != ModuleStatus::Planned {
            if module.paths.is_empty() {
                errors.push(format!(
                    "module {} is {} but owns no paths",
                    module.id,
                    status_name(module.status)
                ));
            } else if !module.paths.iter().any(|path| root.join(path).exists()) {
                errors.push(format!(
                    "module {} is {} but none of its paths exist",
                    module.id,
                    status_name(module.status)
                ));
            }
        }
    }
    if spec.modules.is_empty() {
        errors.push("at least one module is required".into());
    }

    for relation in &spec.relations {
        if !module_ids.contains(relation.from.as_str()) {
            errors.push(format!(
                "relation references unknown module {} → {}",
                relation.from, relation.to
            ));
        }
        if !module_ids.contains(relation.to.as_str()) {
            errors.push(format!(
                "relation references unknown module {} → {}",
                relation.from, relation.to
            ));
        }
    }

    let doc_ids = docs
        .iter()
        .map(|doc| doc.front.id.as_str())
        .collect::<HashSet<_>>();
    let mut seen_docs = HashSet::new();
    for doc in docs {
        if !seen_docs.insert(doc.front.id.as_str()) {
            errors.push(format!("duplicate doc id {}", doc.front.id));
        }
        if doc.front.kind.trim().is_empty() {
            errors.push(format!("doc {} has an empty kind", doc.front.id));
        }
        if doc.front.kind == "adr" {
            let status = doc.front.status.as_deref().unwrap_or("");
            if !matches!(
                status,
                "proposed" | "accepted" | "superseded" | "rejected" | "draft"
            ) {
                errors.push(format!(
                    "doc {} is an ADR with invalid status {:?}",
                    doc.front.id, status
                ));
            }
        }
    }

    let mut card_ids = HashSet::new();
    for card in cards {
        let front = &card.front;
        if !card_ids.insert(front.id.as_str()) {
            errors.push(format!("duplicate card id {}", front.id));
        }
        if !milestone_ids.contains(front.milestone.as_str()) {
            errors.push(format!(
                "card {} references unknown milestone {}",
                front.id, front.milestone
            ));
        }
        if let Some(module) = &front.module {
            if !module_ids.contains(module.as_str()) {
                errors.push(format!(
                    "card {} references unknown module {}",
                    front.id, module
                ));
            }
        }
        for dependency in &front.depends_on {
            if dependency == &front.id {
                errors.push(format!("card {} depends on itself", front.id));
            } else if !cards.iter().any(|other| &other.front.id == dependency) {
                errors.push(format!(
                    "card {} references unknown dependency {}",
                    front.id, dependency
                ));
            }
        }
        for doc in &front.docs {
            if !doc_ids.contains(doc.as_str()) {
                errors.push(format!("card {} references unknown doc {}", front.id, doc));
            }
        }
        if !valid_date(&front.updated) {
            errors.push(format!(
                "card {} has invalid updated date {:?}",
                front.id, front.updated
            ));
        }
        if let Some(completed) = &front.completed {
            if !valid_date(completed) {
                errors.push(format!(
                    "card {} has invalid completed date {:?}",
                    front.id, completed
                ));
            }
        }
        if front.status == CardStatus::Done && front.completed.is_none() {
            errors.push(format!(
                "card {} is done but has no completed date",
                front.id
            ));
        }
        if front.status == CardStatus::Blocked
            && front
                .blocked_reason
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            errors.push(format!(
                "card {} is blocked but has no blocked_reason",
                front.id
            ));
        }
        if let Some(priority) = &front.priority {
            if !matches!(priority.as_str(), "P0" | "P1" | "P2") {
                errors.push(format!(
                    "card {} has invalid priority {:?}",
                    front.id, priority
                ));
            }
        }
    }

    for event in &spec.events {
        if !valid_date(&event.date) {
            errors.push(format!(
                "timeline event {:?} has invalid date {:?}",
                event.title, event.date
            ));
        }
        if event.title.trim().is_empty() {
            errors.push("timeline event has an empty title".into());
        }
    }

    if !errors.is_empty() {
        let mut message = format!("dashboard validation failed ({} issue(s)):\n", errors.len());
        for error in errors {
            message.push_str("  - ");
            message.push_str(&error);
            message.push('\n');
        }
        bail!(message);
    }

    Ok(())
}

fn status_name(status: ModuleStatus) -> &'static str {
    match status {
        ModuleStatus::Planned => "planned",
        ModuleStatus::Active => "active",
        ModuleStatus::Done => "done",
    }
}

fn valid_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return false;
    }

    let month = value[5..7].parse::<u32>().unwrap_or(0);
    let day = value[8..10].parse::<u32>().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}
