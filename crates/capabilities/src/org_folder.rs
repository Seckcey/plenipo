//! The organization folder (Phase 25, ADR-205): one folder for each organization, which holds a
//! folder for each department and project, their **Files** (finished documents) and **Scratch
//! pads** (each agent's notes and drafts). Plenipo makes these itself, not an AI, so the shape is
//! always right:
//!
//! ```text
//! <organization>\
//!   Read me.md
//!   <department>\
//!     Files\
//!     Scratch pads\<agent>\          the department's head and agents on no project
//!     <project>\
//!       Files\
//!       Scratch pads\<agent>\        the project's Supervisor and its team
//!   Scratch pads\<agent>\            agents in no department
//! ```
//!
//! Each folder is recorded once in the Ledger (`folders`) and stays where it was made: a new name
//! never moves it, and one that has gone missing is made again at its recorded path, never
//! somewhere else. A folder already at a place is used only when it is an empty, ordinary folder;
//! otherwise the name gets " (2)". Nothing here ever deletes or moves a folder.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use plenipo_guard::places::{folder_name, link_on_the_way};
use plenipo_ledger::{
    Folder, FolderKind, FolderOrigin, Ledger, LedgerEvent, NewFolder, OrgRecords, Position,
};

/// The file at the top of the organization folder that says what it is.
pub const READ_ME: &str = "Read me.md";
/// A department's or a project's finished documents.
pub const FILES: &str = "Files";
/// The folder of each agent's scratch pad.
pub const SCRATCH_PADS: &str = "Scratch pads";
/// Recorded as the one who acted when Plenipo makes a folder on its own.
pub const KEEPER: &str = "plenipo";

/// The most " (n)" names tried before giving up on a folder.
const MAX_TRIES: u32 = 99;

/// What the organization folder's `Read me.md` says, in plain words.
pub fn read_me(organization: &str) -> String {
    format!(
        "# {organization}\n\n\
         This is {organization}'s organization folder. Plenipo keeps it in order.\n\n\
         - Each department has a folder here. Inside it:\n  \
         - **Files**: the department's finished documents.\n  \
         - **Scratch pads**: a folder for each of its agents' notes and drafts.\n  \
         - A folder for each of its projects, with the project's own **Files** and **Scratch \
         pads**.\n\
         - Plenipo makes these folders itself. You can keep your own files here too.\n\
         - Plenipo backs up its record of {organization} every day. This folder is ordinary \
         files: back it up the way you back up your Documents (for example with OneDrive or File \
         History).\n\n\
         Made by Plenipo, from 8 West Ventures, LLC.\n"
    )
}

/// An empty, ordinary folder (not a link or junction), or nothing at all, is free to use.
fn free(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Err(e) => e.kind() == std::io::ErrorKind::NotFound,
        Ok(meta) => {
            meta.is_dir()
                && !meta.file_type().is_symlink()
                && std::fs::read_dir(path).is_ok_and(|mut d| d.next().is_none())
        }
    }
}

/// `<parent>\<name>`, or `<name> (2)` and on when that place is taken: by a file, a link, a
/// folder with something in it, or a place in `taken`.
fn first_free(parent: &Path, name: &str, taken: &dyn Fn(&Path) -> bool) -> Option<PathBuf> {
    (1..=MAX_TRIES)
        .map(|n| {
            if n == 1 {
                parent.join(name)
            } else {
                parent.join(format!("{name} ({n})"))
            }
        })
        .find(|p| !taken(p) && free(p))
}

/// Where a new organization's folder goes by default: `<base>\<its name>`, or "(2)" and on when
/// that is taken (ADR-205 §2.1). `base` is `Documents\Plenipo`.
pub fn suggest(base: &Path, organization: &str) -> PathBuf {
    let name = folder_name(organization, "Organization");
    first_free(base, &name, &|_| false).unwrap_or_else(|| base.join(name))
}

/// Make the organization folder at `path` (Guard's `place_problem` has checked the place), record
/// it, and write its `Read me.md`. A folder already there is used only when it is empty.
pub fn create(
    ledger: &Ledger,
    path: &Path,
    organization: &str,
    actor: &str,
) -> Result<Folder, String> {
    if let Some(existing) = ledger.organization_folder().map_err(|e| e.to_string())? {
        return Err(format!(
            "this organization already has its folder, {}",
            existing.path
        ));
    }
    if !free(path) {
        return Err(format!(
            "{} already has something in it. Choose an empty folder, or a new one.",
            path.display()
        ));
    }
    let origin = if path.exists() {
        FolderOrigin::Adopted
    } else {
        FolderOrigin::Made
    };
    std::fs::create_dir_all(path)
        .map_err(|e| format!("Plenipo couldn't make {}: {e}", path.display()))?;
    let folder = ledger
        .record_folder(
            &NewFolder {
                kind: FolderKind::Organization,
                ref_id: None,
                path: path.display().to_string(),
            },
            origin,
            actor,
        )
        .map_err(|e| e.to_string())?;
    let read_me_path = path.join(READ_ME);
    if !read_me_path.exists() {
        // The folder stands without it: a Read me that can't be written is only logged.
        if let Err(e) = std::fs::write(&read_me_path, read_me(organization)) {
            log::warn!("could not write the organization folder's {READ_ME}: {e}");
        }
    }
    Ok(folder)
}

/// What one pass of [`keep`] did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Kept {
    /// Folders made (or empty ones taken) and recorded.
    pub made: Vec<String>,
    /// Recorded folders that had gone missing and were made again at their path.
    pub remade: Vec<String>,
    /// What could not be done, in plain words.
    pub problems: Vec<String>,
}

/// Who belongs where: each position's department and project, from its reporting line (the
/// nearest department head or project Supervisor at or above it).
struct Chart<'a> {
    positions: HashMap<&'a str, &'a Position>,
    heads: HashMap<&'a str, &'a str>,
    coordinators: HashMap<&'a str, &'a str>,
}

impl<'a> Chart<'a> {
    fn new(records: &'a OrgRecords) -> Self {
        Self {
            positions: records
                .positions
                .iter()
                .map(|p| (p.id.as_str(), p))
                .collect(),
            heads: records
                .departments
                .iter()
                .filter_map(|d| Some((d.head_position_id.as_deref()?, d.id.as_str())))
                .collect(),
            coordinators: records
                .projects
                .iter()
                .filter_map(|p| Some((p.coordinator_position_id.as_deref()?, p.id.as_str())))
                .collect(),
        }
    }

    /// The nearest position at or above `id` found in `map`.
    fn nearest(&self, id: &str, map: &HashMap<&'a str, &'a str>) -> Option<&'a str> {
        let mut current = Some(id);
        let mut seen = HashSet::new();
        while let Some(at) = current {
            if let Some(found) = map.get(at) {
                return Some(found);
            }
            if !seen.insert(at.to_owned()) {
                return None;
            }
            current = self.positions.get(at).and_then(|p| p.reports_to.as_deref());
        }
        None
    }

    fn department_of(&self, id: &str) -> Option<&'a str> {
        self.nearest(id, &self.heads)
    }

    fn project_of(&self, id: &str) -> Option<&'a str> {
        self.nearest(id, &self.coordinators)
    }
}

/// One pass of keeping the organization folder's shape (ADR-205 §2.2): make each folder the org
/// chart needs and has not got yet, and make again any recorded folder that has gone missing.
/// An organization with no organization folder is left alone.
struct Keeper<'a> {
    ledger: &'a Ledger,
    recorded: Vec<Folder>,
    trusted: Vec<PathBuf>,
    kept: Kept,
}

impl Keeper<'_> {
    fn taken(&self, path: &Path) -> bool {
        let shown = path.display().to_string();
        self.recorded
            .iter()
            .any(|f| f.path.eq_ignore_ascii_case(&shown))
    }

    /// The folder of `kind` for `ref_id`: as recorded (made again when missing), or made now in
    /// `parent` under `name`. `None` when it can't be had; the reason is in `problems`.
    fn place(
        &mut self,
        kind: FolderKind,
        ref_id: Option<&str>,
        parent: &Path,
        name: &str,
    ) -> Option<PathBuf> {
        if let Some(f) = self
            .recorded
            .iter()
            .find(|f| f.kind == kind && f.ref_id.as_deref() == ref_id)
            .cloned()
        {
            let path = PathBuf::from(&f.path);
            self.make_again(&f, &path);
            return Some(path);
        }
        if let Some(link) = link_on_the_way(parent, &self.trusted) {
            self.kept.problems.push(format!(
                "{} is a shortcut to another place, so Plenipo didn't make a folder in it",
                link.display()
            ));
            return None;
        }
        let Some(path) = first_free(parent, name, &|p| self.taken(p)) else {
            self.kept.problems.push(format!(
                "Plenipo couldn't find a free name for the {name} folder in {}",
                parent.display()
            ));
            return None;
        };
        let origin = if path.exists() {
            FolderOrigin::Adopted
        } else {
            FolderOrigin::Made
        };
        if let Err(e) = std::fs::create_dir_all(&path) {
            self.kept
                .problems
                .push(format!("Plenipo couldn't make {}: {e}", path.display()));
            return None;
        }
        match self.ledger.record_folder(
            &NewFolder {
                kind,
                ref_id: ref_id.map(str::to_owned),
                path: path.display().to_string(),
            },
            origin,
            KEEPER,
        ) {
            Ok(folder) => {
                self.kept.made.push(folder.path.clone());
                self.recorded.push(folder);
                Some(path)
            }
            Err(e) => {
                self.kept
                    .problems
                    .push(format!("Plenipo couldn't record {}: {e}", path.display()));
                None
            }
        }
    }

    /// A recorded folder that has gone missing is made again at its recorded path (the
    /// reviewer's O6): never somewhere else, and never through a link.
    fn make_again(&mut self, folder: &Folder, path: &Path) {
        if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            self.kept.problems.push(format!(
                "{} is now a shortcut to another place, so Plenipo leaves it as it is",
                path.display()
            ));
            return;
        }
        if path.is_dir() {
            return;
        }
        if path.exists() {
            self.kept.problems.push(format!(
                "{} is no longer a folder, so Plenipo left it as it is",
                path.display()
            ));
            return;
        }
        if let Some(link) = link_on_the_way(path, &self.trusted) {
            self.kept.problems.push(format!(
                "{} is now a shortcut to another place, so Plenipo didn't make {} again",
                link.display(),
                path.display()
            ));
            return;
        }
        match std::fs::create_dir_all(path) {
            Ok(()) => {
                if let Err(e) = self.ledger.record_folder_remade(&folder.id, KEEPER) {
                    log::warn!("could not record a folder made again: {e}");
                }
                self.kept.remade.push(folder.path.clone());
            }
            Err(e) => self.kept.problems.push(format!(
                "{} was missing and Plenipo couldn't make it again there: {e}",
                path.display()
            )),
        }
    }
}

/// Keep the organization folder's shape (see [`Keeper`]). `trusted` are the system's own folders
/// (the profile's and Documents), at and above which a junction is not looked at.
pub fn keep(ledger: &Ledger, trusted: &[PathBuf]) -> Kept {
    let mut keeper = Keeper {
        ledger,
        recorded: match ledger.folders() {
            Ok(f) => f,
            Err(e) => {
                return Kept {
                    problems: vec![format!("Plenipo couldn't read its folders: {e}")],
                    ..Kept::default()
                }
            }
        },
        trusted: trusted.to_vec(),
        kept: Kept::default(),
    };
    let Some(org) = keeper
        .recorded
        .iter()
        .find(|f| f.kind == FolderKind::Organization)
        .cloned()
    else {
        return keeper.kept;
    };
    let root = PathBuf::from(&org.path);
    keeper.make_again(&org, &root);
    if !root.is_dir() {
        return keeper.kept;
    }
    let records = match ledger.org_records() {
        Ok(r) => r,
        Err(e) => {
            keeper
                .kept
                .problems
                .push(format!("Plenipo couldn't read the org chart: {e}"));
            return keeper.kept;
        }
    };
    let active =
        |archived: Option<u64>, deleted: Option<u64>| archived.is_none() && deleted.is_none();
    let mut pads: HashMap<String, PathBuf> = HashMap::new();
    let mut department_folders: HashMap<String, PathBuf> = HashMap::new();
    for d in records
        .departments
        .iter()
        .filter(|d| active(d.archived_at, d.deleted_at))
    {
        let name = folder_name(&d.name, "Department");
        let Some(dir) = keeper.place(FolderKind::Department, Some(&d.id), &root, &name) else {
            continue;
        };
        keeper.place(FolderKind::DepartmentFiles, Some(&d.id), &dir, FILES);
        if let Some(p) = keeper.place(FolderKind::ScratchPads, Some(&d.id), &dir, SCRATCH_PADS) {
            pads.insert(d.id.clone(), p);
        }
        department_folders.insert(d.id.clone(), dir);
    }
    for p in records
        .projects
        .iter()
        .filter(|p| active(p.archived_at, p.deleted_at))
    {
        let parent = p
            .department_id
            .as_ref()
            .and_then(|d| department_folders.get(d))
            .cloned()
            .unwrap_or_else(|| root.clone());
        let name = folder_name(&p.name, "Project");
        let Some(dir) = keeper.place(FolderKind::Project, Some(&p.id), &parent, &name) else {
            continue;
        };
        keeper.place(FolderKind::ProjectFiles, Some(&p.id), &dir, FILES);
        if let Some(pad) = keeper.place(FolderKind::ScratchPads, Some(&p.id), &dir, SCRATCH_PADS) {
            pads.insert(p.id.clone(), pad);
        }
    }
    let chart = Chart::new(&records);
    let mut own_pads: Option<PathBuf> = None;
    for position in records
        .positions
        .iter()
        .filter(|p| active(p.archived_at, p.deleted_at))
    {
        let home = chart
            .project_of(&position.id)
            .and_then(|p| pads.get(p))
            .or_else(|| chart.department_of(&position.id).and_then(|d| pads.get(d)))
            .cloned();
        let container = match home {
            Some(c) => c,
            None => {
                if own_pads.is_none() {
                    own_pads = keeper.place(FolderKind::ScratchPads, None, &root, SCRATCH_PADS);
                }
                let Some(c) = own_pads.clone() else {
                    continue;
                };
                c
            }
        };
        let name = folder_name(&position.title, "Agent");
        keeper.place(
            FolderKind::ScratchPad,
            Some(&position.id),
            &container,
            &name,
        );
    }
    keeper.kept
}

/// Events after which the org chart may need a folder: a department, project, or position made,
/// moved, brought back, or renamed. A worker starting or ending a task never does.
pub fn may_need_folders(event: &LedgerEvent) -> bool {
    let Some(what) = event.event_type.strip_prefix("org.") else {
        return false;
    };
    !matches!(
        what,
        "worker_spawned"
            | "worker_started"
            | "worker_retired"
            | "agent_routed"
            | "agent_joined_objective"
            | "agent_going_home"
            | "agent_lent"
            | "agent_returned"
            | "hire_needed"
            | "oversight_ended"
            | "settings_changed"
    )
}

/// Keeps one organization's folder in shape in the background: each nudge runs [`keep`] on a
/// thread of its own, one pass at a time, and a nudge during a pass runs one more after it.
pub struct FolderKeeper {
    ledger: Arc<Ledger>,
    trusted: Vec<PathBuf>,
    running: AtomicBool,
    again: AtomicBool,
}

impl FolderKeeper {
    pub fn new(ledger: Arc<Ledger>, trusted: Vec<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            ledger,
            trusted,
            running: AtomicBool::new(false),
            again: AtomicBool::new(false),
        })
    }

    /// One pass now, on this thread.
    pub fn keep_now(&self) -> Kept {
        let kept = keep(&self.ledger, &self.trusted);
        for problem in &kept.problems {
            log::warn!("organization folder: {problem}");
        }
        kept
    }

    /// Keep the folder in shape soon, on a thread of its own.
    pub fn nudge(self: &Arc<Self>) {
        self.again.store(true, Ordering::SeqCst);
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let me = self.clone();
        let started = std::thread::Builder::new()
            .name("plenipo-org-folder".into())
            .spawn(move || loop {
                while me.again.swap(false, Ordering::SeqCst) {
                    me.keep_now();
                }
                me.running.store(false, Ordering::SeqCst);
                // A nudge after the last pass but before `running` cleared saw it still running
                // and left: take it now, unless another thread already has.
                if !me.again.load(Ordering::SeqCst) || me.running.swap(true, Ordering::SeqCst) {
                    break;
                }
            });
        if let Err(e) = started {
            self.running.store(false, Ordering::SeqCst);
            log::warn!("could not start keeping the organization folder: {e}");
        }
    }

    /// Nudge after every committed event that may need a folder.
    pub fn listen(self: &Arc<Self>) {
        let me = Arc::downgrade(self);
        self.ledger.add_listener(Arc::new(move |event| {
            if may_need_folders(event) {
                if let Some(me) = me.upgrade() {
                    me.nudge();
                }
            }
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plenipo_ledger::{NewPosition, ProjectSettings};

    struct Org {
        ledger: Ledger,
        _dir: tempfile::TempDir,
        base: PathBuf,
    }

    fn org() -> Org {
        let dir = tempfile::tempdir().unwrap();
        let base = dunce::canonicalize(dir.path()).unwrap();
        let ledger = Ledger::open_in_memory().unwrap();
        let templates = plenipo_workforce::templates::role_templates();
        ledger.ensure_roles(&templates, "owner").unwrap();
        Org {
            ledger,
            _dir: dir,
            base,
        }
    }

    fn role(l: &Ledger, name: &str) -> String {
        l.org_records()
            .unwrap()
            .roles
            .into_iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("no role {name}"))
            .id
    }

    fn position(l: &Ledger, title: &str, role_name: &str, reports_to: Option<&str>) -> NewPosition {
        NewPosition {
            title: title.into(),
            role_id: role(l, role_name),
            reports_to: reports_to.map(str::to_owned),
            ..NewPosition::default()
        }
    }

    /// Development (its Manager), the Website project (its Supervisor, a Senior Developer on its
    /// team), an on-call QA Engineer under the Manager, and a Chief of Staff (a VP) in no department.
    fn chart(l: &Ledger) -> (String, String, Vec<String>) {
        let (dept, head) = l
            .create_department_with_head(
                "Development",
                "",
                &position(l, "Development Manager", "Manager", None),
                "owner",
            )
            .unwrap();
        let (project, supervisor) = l
            .create_project_with_coordinator(
                &dept.id,
                &ProjectSettings {
                    name: "Website".into(),
                    ..ProjectSettings::default()
                },
                &position(l, "Website Supervisor", "Supervisor", None),
                "owner",
            )
            .unwrap();
        let (dev, _) = l
            .create_position(
                &position(
                    l,
                    "Senior Developer",
                    "Senior Developer",
                    Some(&supervisor.id),
                ),
                "owner",
            )
            .unwrap();
        let (on_call, _) = l
            .create_position(
                &position(l, "QA Engineer", "QA Engineer", Some(&head.id)),
                "owner",
            )
            .unwrap();
        let (researcher, _) = l
            .create_position(&position(l, "Chief of Staff", "VP", None), "owner")
            .unwrap();
        (
            dept.id,
            project.id,
            vec![head.id, supervisor.id, dev.id, on_call.id, researcher.id],
        )
    }

    fn pad_of(l: &Ledger, position: &str) -> PathBuf {
        PathBuf::from(
            l.folder(FolderKind::ScratchPad, Some(position))
                .unwrap()
                .unwrap()
                .path,
        )
    }

    #[test]
    fn an_organization_with_no_folder_is_left_alone() {
        let o = org();
        chart(&o.ledger);
        assert_eq!(keep(&o.ledger, &[]), Kept::default());
        assert!(o.ledger.folders().unwrap().is_empty());
    }

    #[test]
    fn the_shape_follows_the_org_chart() {
        let o = org();
        let root = o.base.join("Acme Co");
        create(&o.ledger, &root, "Acme Co", "owner").unwrap();
        assert!(std::fs::read_to_string(root.join(READ_ME))
            .unwrap()
            .contains("8 West Ventures, LLC"));
        let (dept, project, positions) = chart(&o.ledger);
        let kept = keep(&o.ledger, &[]);
        assert!(kept.problems.is_empty(), "{kept:?}");
        let dev = root.join("Development");
        let web = dev.join("Website");
        for dir in [
            dev.join(FILES),
            dev.join(SCRATCH_PADS).join("Development Manager"),
            dev.join(SCRATCH_PADS).join("QA Engineer"),
            web.join(FILES),
            web.join(SCRATCH_PADS).join("Website Supervisor"),
            web.join(SCRATCH_PADS).join("Senior Developer"),
            root.join(SCRATCH_PADS).join("Chief of Staff"),
        ] {
            assert!(dir.is_dir(), "{dir:?} in {kept:?}");
        }
        assert_eq!(
            o.ledger
                .folder(FolderKind::Project, Some(&project))
                .unwrap()
                .unwrap()
                .path,
            web.display().to_string()
        );
        assert_eq!(
            o.ledger
                .folder(FolderKind::DepartmentFiles, Some(&dept))
                .unwrap()
                .unwrap()
                .path,
            dev.join(FILES).display().to_string()
        );
        assert_eq!(
            pad_of(&o.ledger, &positions[2]),
            web.join(SCRATCH_PADS).join("Senior Developer")
        );
        // A second pass has nothing to do.
        assert_eq!(keep(&o.ledger, &[]), Kept::default());
    }

    #[test]
    fn names_are_cleaned_and_taken_places_get_a_number() {
        let o = org();
        let root = o.base.join("Acme");
        create(&o.ledger, &root, "Acme", "owner").unwrap();
        // The owner already keeps something in a folder named like the department.
        std::fs::create_dir_all(root.join("Sales")).unwrap();
        std::fs::write(root.join("Sales").join("mine.txt"), "the owner's").unwrap();
        // And an empty folder named like another: it is used as it is.
        std::fs::create_dir_all(root.join("Design")).unwrap();
        o.ledger
            .create_department_with_head(
                "Sales",
                "",
                &position(&o.ledger, "Sales Manager", "Manager", None),
                "owner",
            )
            .unwrap();
        let (design, _) = o
            .ledger
            .create_department_with_head(
                "Design",
                "",
                &position(&o.ledger, "Design Manager", "Manager", None),
                "owner",
            )
            .unwrap();
        let (odd, _) = o
            .ledger
            .create_department_with_head(
                "R&D: <new>/ideas.",
                "",
                &position(&o.ledger, "Ideas Manager", "Manager", None),
                "owner",
            )
            .unwrap();
        let kept = keep(&o.ledger, &[]);
        assert!(kept.problems.is_empty(), "{kept:?}");
        assert!(root.join("Sales (2)").join(FILES).is_dir());
        assert_eq!(
            std::fs::read_to_string(root.join("Sales").join("mine.txt")).unwrap(),
            "the owner's",
            "the owner's folder is left alone"
        );
        let design_folder = o
            .ledger
            .folder(FolderKind::Department, Some(&design.id))
            .unwrap()
            .unwrap();
        assert_eq!(
            design_folder.path,
            root.join("Design").display().to_string()
        );
        assert!(!design_folder.made_by_plenipo);
        assert_eq!(
            o.ledger
                .folder(FolderKind::Department, Some(&odd.id))
                .unwrap()
                .unwrap()
                .path,
            root.join("R&D new ideas").display().to_string()
        );
    }

    #[test]
    fn a_missing_folder_is_made_again_where_it_was_and_a_rename_moves_nothing() {
        let o = org();
        let root = o.base.join("Acme");
        create(&o.ledger, &root, "Acme", "owner").unwrap();
        let (dept, _, positions) = chart(&o.ledger);
        keep(&o.ledger, &[]);
        let pad = pad_of(&o.ledger, &positions[1]);
        std::fs::remove_dir_all(root.join("Development").join("Website")).unwrap();
        // Renamed in Plenipo: the folder keeps its recorded place.
        o.ledger
            .update_department_details(&dept, "Engineering", "", true, "owner")
            .unwrap();
        let kept = keep(&o.ledger, &[]);
        assert!(kept.problems.is_empty(), "{kept:?}");
        assert!(kept.remade.contains(&pad.display().to_string()), "{kept:?}");
        assert!(pad.is_dir());
        assert!(!root.join("Engineering").exists());
        let remade = o
            .ledger
            .recent_events(50)
            .unwrap()
            .into_iter()
            .filter(|e| e.event_type == "folder.remade")
            .count();
        assert!(
            remade >= 3,
            "the project, its Files, Scratch pads, and pads: {remade}"
        );
    }

    /// The reviewer's O2 and O6: a recorded folder that became a junction or link to another
    /// place is not made again through it, and nothing is made inside a linked folder.
    #[test]
    fn nothing_is_made_through_a_junction() {
        let o = org();
        let root = o.base.join("Acme");
        create(&o.ledger, &root, "Acme", "owner").unwrap();
        let (_, _, positions) = chart(&o.ledger);
        keep(&o.ledger, &[]);
        let elsewhere = o.base.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        let pads = root.join("Development").join("Website").join(SCRATCH_PADS);
        std::fs::remove_dir_all(&pads).unwrap();
        make_link(&elsewhere, &pads);
        let kept = keep(&o.ledger, &[]);
        assert!(
            kept.problems.iter().any(|p| p.contains("shortcut")),
            "{kept:?}"
        );
        assert!(
            std::fs::read_dir(&elsewhere).unwrap().next().is_none(),
            "nothing was made in the place the junction leads to"
        );
        let _ = pad_of(&o.ledger, &positions[1]);
    }

    #[cfg(windows)]
    fn make_link(target: &Path, link: &Path) {
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[cfg(unix)]
    fn make_link(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    #[test]
    fn the_organization_folder_must_be_new_or_empty() {
        let o = org();
        let full = o.base.join("Full");
        std::fs::create_dir_all(&full).unwrap();
        std::fs::write(full.join("x.txt"), "x").unwrap();
        assert!(create(&o.ledger, &full, "Acme", "owner").is_err());
        let empty = o.base.join("Empty");
        std::fs::create_dir_all(&empty).unwrap();
        let f = create(&o.ledger, &empty, "Acme", "owner").unwrap();
        assert!(!f.made_by_plenipo);
        assert!(create(&o.ledger, &o.base.join("Again"), "Acme", "owner").is_err());
    }

    #[test]
    fn a_suggestion_skips_names_that_are_taken() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        assert_eq!(suggest(base, "Acme: Co"), base.join("Acme Co"));
        std::fs::create_dir_all(base.join("Acme Co")).unwrap();
        // An empty folder of that name is free to use.
        assert_eq!(suggest(base, "Acme Co"), base.join("Acme Co"));
        std::fs::write(base.join("Acme Co").join("x"), "").unwrap();
        assert_eq!(suggest(base, "Acme Co"), base.join("Acme Co (2)"));
        assert_eq!(suggest(base, ""), base.join("Organization"));
    }

    #[test]
    fn only_changes_to_the_org_chart_nudge_the_keeper() {
        let event = |t: &str| LedgerEvent {
            seq: 1,
            id: "e".into(),
            task_id: None,
            execution_id: None,
            source: "owner".into(),
            destination: None,
            event_type: t.into(),
            payload: serde_json::Value::Null,
            created_at: 0,
        };
        for yes in [
            "org.department_created",
            "org.project_created",
            "org.position_created",
            "org.project_reassigned",
            "org.position_updated",
        ] {
            assert!(may_need_folders(&event(yes)), "{yes}");
        }
        for no in [
            "org.worker_spawned",
            "task.created",
            "folder.made",
            "org.settings_changed",
        ] {
            assert!(!may_need_folders(&event(no)), "{no}");
        }
    }

    #[test]
    fn the_keeper_follows_the_ledger_in_the_background() {
        let o = org();
        let ledger = Arc::new(o.ledger);
        create(&ledger, &o.base.join("Acme"), "Acme", "owner").unwrap();
        let keeper = FolderKeeper::new(ledger.clone(), vec![]);
        keeper.listen();
        chart(&ledger);
        let pad = o
            .base
            .join("Acme")
            .join(SCRATCH_PADS)
            .join("Chief of Staff");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !pad.is_dir() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(pad.is_dir());
    }
}
