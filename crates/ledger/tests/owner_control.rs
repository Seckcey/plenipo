//! Phase 17 in the Ledger: specialties (ADR-042); archive, bring back, and delete for good
//! (ADR-043); experience and the Workforce (ADR-045).

use std::collections::HashMap;

use plenipo_ledger::workforce::Providers;
use plenipo_ledger::{
    Ledger, LedgerError, NewLessons, NewPosition, NewTask, OversightKind, PositionPatch,
    PositionState, ProjectSettings, RoleTemplate, RoleType, SaveAgent, SpecialtyFields,
    SpecialtyTemplate, TaskState,
};
use serde_json::{json, Value};

const OWNER: &str = "owner";

fn template(name: &'static str, role_type: RoleType, persistent: bool) -> RoleTemplate {
    RoleTemplate {
        name,
        description: "",
        role_type,
        persistent,
        metadata: json!({ "template": true }),
        formerly: &[],
    }
}

struct World {
    l: Ledger,
    roles: HashMap<&'static str, String>,
}

fn world() -> World {
    let l = Ledger::open_in_memory().unwrap();
    let roles = l
        .ensure_roles(
            &[
                template("VP", RoleType::Superintendent, true),
                template("Manager", RoleType::DepartmentManager, true),
                template("Supervisor", RoleType::ProjectCoordinator, true),
                template("Senior Developer", RoleType::Worker, false),
                template("Code Reviewer", RoleType::Worker, false),
                template("Staff Engineer", RoleType::Worker, true),
            ],
            "plenipo",
        )
        .unwrap();
    let roles = roles
        .into_iter()
        .map(|r| {
            let name: &'static str = match r.name.as_str() {
                "VP" => "VP",
                "Manager" => "Manager",
                "Supervisor" => "Supervisor",
                "Senior Developer" => "Senior Developer",
                "Code Reviewer" => "Code Reviewer",
                _ => "Staff Engineer",
            };
            (name, r.id)
        })
        .collect();
    World { l, roles }
}

fn new_position(role: &str, title: &str, reports_to: Option<&str>) -> NewPosition {
    NewPosition {
        title: title.into(),
        role_id: role.into(),
        reports_to: reports_to.map(str::to_owned),
        staffed: true,
        ..NewPosition::default()
    }
}

fn settings(name: &str) -> ProjectSettings {
    ProjectSettings {
        name: name.into(),
        description: "A project".into(),
        allowed_runtimes: vec!["claude-code".into(), "codex".into()],
        ..ProjectSettings::default()
    }
}

/// Development (manager), Website (supervisor) with a developer and a reviewer, and a staff
/// engineer reporting to the manager directly.
struct Org {
    dept: String,
    manager: String,
    project: String,
    supervisor: String,
    developer: String,
    reviewer: String,
    staff: String,
}

fn org(w: &World) -> Org {
    let l = &w.l;
    let (dept, manager) = l
        .create_department_with_head(
            "Development",
            "",
            &new_position(&w.roles["Manager"], "Development Manager", None),
            OWNER,
        )
        .unwrap();
    let (project, supervisor) = l
        .create_project_with_coordinator(
            &dept.id,
            &settings("Website"),
            &new_position(&w.roles["Supervisor"], "Website Supervisor", None),
            OWNER,
        )
        .unwrap();
    let (developer, _) = l
        .create_position(
            &new_position(
                &w.roles["Senior Developer"],
                "Website Developer",
                Some(&supervisor.id),
            ),
            OWNER,
        )
        .unwrap();
    let (reviewer, _) = l
        .create_position(
            &new_position(
                &w.roles["Code Reviewer"],
                "Website Reviewer",
                Some(&supervisor.id),
            ),
            OWNER,
        )
        .unwrap();
    let (staff, _) = l
        .create_position(
            &new_position(
                &w.roles["Staff Engineer"],
                "Staff Engineer",
                Some(&manager.id),
            ),
            OWNER,
        )
        .unwrap();
    Org {
        dept: dept.id,
        manager: manager.id,
        project: project.id,
        supervisor: supervisor.id,
        developer: developer.id,
        reviewer: reviewer.id,
        staff: staff.id,
    }
}

fn err<T: std::fmt::Debug>(r: Result<T, LedgerError>) -> String {
    match r {
        Err(LedgerError::InvalidInput(m)) => m,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn types(l: &Ledger) -> Vec<String> {
    l.recent_events(1000)
        .unwrap()
        .into_iter()
        .map(|e| e.event_type)
        .collect()
}

fn state(l: &Ledger, id: &str) -> PositionState {
    l.position(id).unwrap().unwrap().state
}

fn no_providers() -> Providers {
    Providers::new()
}

/// A task of `position_id` that is still going.
fn busy(l: &Ledger, position_id: &str) -> String {
    l.create_task(
        NewTask {
            requested_by: OWNER.into(),
            objective: "work".into(),
            metadata: json!({ "workforce": { "positionId": position_id } }),
            ..NewTask::default()
        },
        OWNER,
    )
    .unwrap()
    .id
}

/// A finished task of `position_id`.
fn done(l: &Ledger, position_id: &str) -> String {
    let id = busy(l, position_id);
    l.transition_task(&id, TaskState::Running, OWNER, None)
        .unwrap();
    l.transition_task(&id, TaskState::Succeeded, OWNER, None)
        .unwrap();
    id
}

// ---- Specialties (ADR-042) -----------------------------------------------------------------

fn builtins() -> Vec<SpecialtyTemplate> {
    vec![
        SpecialtyTemplate {
            role: "Senior Developer",
            name: "Database",
            title: "Database Developer",
            metadata: json!({ "job": { "duties": ["design tables"] } }),
        },
        SpecialtyTemplate {
            role: "Senior Developer",
            name: "Front-end",
            title: "Front-end Developer",
            metadata: json!({ "job": { "duties": ["build pages"] } }),
        },
        SpecialtyTemplate {
            role: "No Such Role",
            name: "Anything",
            title: "",
            metadata: json!({}),
        },
    ]
}

#[test]
fn built_in_specialties_are_seeded_once_and_kept_up_to_date() {
    let w = world();
    let first = w.l.ensure_specialties(&builtins(), "plenipo").unwrap();
    assert_eq!(first.len(), 2, "a template for a missing role is left out");
    assert!(first.iter().all(|s| s.built_in()));
    let again = w.l.ensure_specialties(&builtins(), "plenipo").unwrap();
    assert_eq!(first, again, "nothing changes the second time");
    let mut newer = builtins();
    newer[0].metadata = json!({ "job": { "duties": ["design tables and indexes"] } });
    let updated = w.l.ensure_specialties(&newer, "plenipo").unwrap();
    let db = updated.iter().find(|s| s.name == "Database").unwrap();
    assert_eq!(db.metadata["job"]["duties"][0], "design tables and indexes");
    assert_eq!(
        db.id,
        first.iter().find(|s| s.name == "Database").unwrap().id
    );
    assert!(types(&w.l).iter().any(|t| t == "org.specialty_updated"));
}

#[test]
fn the_owner_adds_changes_and_removes_specialties_on_any_role() {
    let w = world();
    let builtin = w.l.ensure_specialties(&builtins(), "plenipo").unwrap();
    let db = builtin.iter().find(|s| s.name == "Database").unwrap();
    let fields = |name: &str| SpecialtyFields {
        name: name.into(),
        title: format!("{name} Engineer"),
        metadata: json!({ "job": { "duties": ["tune queries"] } }),
    };
    let own =
        w.l.create_specialty(&w.roles["Staff Engineer"], &fields("Performance"), OWNER)
            .unwrap();
    assert!(!own.built_in());
    assert!(err(w
        .l
        .create_specialty(&w.roles["Staff Engineer"], &fields("performance"), OWNER))
    .contains("already has a specialty"));
    // Built-in specialties keep their lines and stay.
    assert!(err(w.l.update_specialty(&db.id, &fields("Data"), OWNER)).contains("built-in"));
    assert!(err(w.l.remove_specialty(&db.id, OWNER)).contains("built-in"));
    let renamed =
        w.l.update_specialty(&own.id, &fields("Speed"), OWNER)
            .unwrap();
    assert_eq!(renamed.name, "Speed");
    // A position with it: removing is refused until it has another.
    let o = org(&w);
    let (staff, _) =
        w.l.create_position(
            &NewPosition {
                specialty_id: Some(own.id.clone()),
                ..new_position(
                    &w.roles["Staff Engineer"],
                    "Speed Engineer",
                    Some(&o.manager),
                )
            },
            OWNER,
        )
        .unwrap();
    let staff = staff.id;
    assert!(err(w.l.remove_specialty(&own.id, OWNER)).contains("Speed Engineer"));
    w.l.update_position(
        &staff,
        &PositionPatch {
            specialty: Some(None),
            ..PositionPatch::default()
        },
        OWNER,
    )
    .unwrap();
    let removed = w.l.remove_specialty(&own.id, OWNER).unwrap();
    assert!(removed.removed_at.is_some());
    // A removed specialty cannot be given again.
    assert!(err(w.l.update_position(
        &staff,
        &PositionPatch {
            specialty: Some(Some(own.id.clone())),
            ..PositionPatch::default()
        },
        OWNER
    ))
    .contains("was removed"));
}

#[test]
fn a_specialty_must_belong_to_the_positions_role_and_changing_it_keeps_the_agent() {
    let w = world();
    let o = org(&w);
    let builtin = w.l.ensure_specialties(&builtins(), "plenipo").unwrap();
    let db = builtin.iter().find(|s| s.name == "Database").unwrap();
    let front = builtin.iter().find(|s| s.name == "Front-end").unwrap();
    // Another role's specialty is refused.
    assert!(err(w.l.create_position(
        &NewPosition {
            specialty_id: Some(db.id.clone()),
            ..new_position(&w.roles["Staff Engineer"], "DBA", Some(&o.manager))
        },
        OWNER
    ))
    .contains("another role"));
    let (dev, _) =
        w.l.create_position(
            &NewPosition {
                specialty_id: Some(db.id.clone()),
                ..new_position(
                    &w.roles["Senior Developer"],
                    "Database Developer",
                    Some(&o.supervisor),
                )
            },
            OWNER,
        )
        .unwrap();
    assert_eq!(dev.specialty_id.as_deref(), Some(db.id.as_str()));
    // A full-time position changes specialty without a new agent.
    let staff_agent = w.l.position_incumbent(&o.staff).unwrap().unwrap();
    let own =
        w.l.create_specialty(
            &w.roles["Staff Engineer"],
            &SpecialtyFields {
                name: "Builds".into(),
                title: String::new(),
                metadata: json!({}),
            },
            OWNER,
        )
        .unwrap();
    let (changed, replacement) =
        w.l.update_position(
            &o.staff,
            &PositionPatch {
                specialty: Some(Some(own.id.clone())),
                ..PositionPatch::default()
            },
            OWNER,
        )
        .unwrap();
    assert!(replacement.is_none());
    assert_eq!(changed.specialty_id.as_deref(), Some(own.id.as_str()));
    assert_eq!(
        w.l.position_incumbent(&o.staff).unwrap().unwrap().id,
        staff_agent.id
    );
    let _ = front;
}

// ---- Archive and bring back (ADR-043) ------------------------------------------------------

#[test]
fn an_agent_comes_back_as_it_was_with_its_assignments() {
    let w = world();
    let o = org(&w);
    let l = &w.l;
    // The reviewer also reviews the staff engineer's team (a full-time lead).
    let (helper, _) = l
        .create_position(
            &new_position(&w.roles["Senior Developer"], "Staff Helper", Some(&o.staff)),
            OWNER,
        )
        .unwrap();
    l.assign_oversight(OversightKind::Review, &o.reviewer, &o.staff, OWNER)
        .unwrap();
    l.archive_position(&helper.id, OWNER).unwrap();
    let before = l.position(&o.reviewer).unwrap().unwrap();
    l.archive_position(&o.reviewer, OWNER).unwrap();
    let archived = l.position(&o.reviewer).unwrap().unwrap();
    assert_eq!(archived.metadata["archive"]["with"], Value::Null);
    assert_eq!(
        archived.metadata["archive"]["oversight"][0]["kind"],
        "review"
    );
    let back = l
        .bring_back_position(&o.reviewer, &no_providers(), OWNER)
        .unwrap();
    assert_eq!(back.state, PositionState::Active);
    assert_eq!(back.title, before.title);
    assert!(back.metadata.get("archive").is_none());
    let records = l.org_records().unwrap();
    assert!(
        records
            .oversight
            .iter()
            .any(|x| x.overseer_id == o.reviewer && x.target_id == o.staff),
        "its review assignment came back"
    );
    assert!(types(l).iter().any(|t| t == "org.position_restored"));
    // Not archived, or already back: refused.
    assert!(
        err(l.bring_back_position(&o.reviewer, &no_providers(), OWNER)).contains("not archived")
    );
}

#[test]
fn a_full_time_agent_gets_a_new_agent_when_it_had_one() {
    let w = world();
    let o = org(&w);
    let l = &w.l;
    let first = l.position_incumbent(&o.staff).unwrap().unwrap();
    l.archive_position(&o.staff, OWNER).unwrap();
    assert!(l.position_incumbent(&o.staff).unwrap().is_none());
    l.bring_back_position(&o.staff, &no_providers(), OWNER)
        .unwrap();
    let now = l.position_incumbent(&o.staff).unwrap().unwrap();
    assert_ne!(now.id, first.id, "a new agent, with a fresh conversation");
    // Vacant when archived: vacant when brought back.
    l.vacate_position(&o.staff, OWNER).unwrap();
    l.archive_position(&o.staff, OWNER).unwrap();
    l.bring_back_position(&o.staff, &no_providers(), OWNER)
        .unwrap();
    assert!(l.position_incumbent(&o.staff).unwrap().is_none());
}

#[test]
fn bringing_back_is_refused_with_what_to_do() {
    let w = world();
    let o = org(&w);
    let l = &w.l;
    l.archive_project(&o.project, OWNER).unwrap();
    // A team member archived with its project comes back with it.
    assert!(
        err(l.bring_back_position(&o.developer, &no_providers(), OWNER))
            .contains("bring the project back")
    );
    // Its supervisor too.
    assert!(err(l.bring_back_position(&o.supervisor, &no_providers(), OWNER)).contains("project"));
    // A title now taken on the team.
    l.archive_position(&o.staff, OWNER).unwrap();
    l.create_position(
        &new_position(
            &w.roles["Staff Engineer"],
            "Staff Engineer",
            Some(&o.manager),
        ),
        OWNER,
    )
    .unwrap();
    assert!(err(l.bring_back_position(&o.staff, &no_providers(), OWNER))
        .contains("already has a team member"));
}

#[test]
fn a_project_comes_back_with_its_team() {
    let w = world();
    let o = org(&w);
    let l = &w.l;
    let supervisor_agent = l.position_incumbent(&o.supervisor).unwrap().unwrap();
    let project = l.archive_project(&o.project, OWNER).unwrap();
    assert!(project.archived_at.is_some());
    assert_eq!(
        l.position(&o.developer).unwrap().unwrap().metadata["archive"]["with"]["id"],
        o.project.as_str()
    );
    let back = l
        .bring_back_project(&o.project, &no_providers(), OWNER)
        .unwrap();
    assert_eq!(back.status, "active");
    assert!(back.archived_at.is_none());
    for id in [&o.supervisor, &o.developer, &o.reviewer] {
        assert_eq!(state(l, id), PositionState::Active);
    }
    let agent = l.position_incumbent(&o.supervisor).unwrap().unwrap();
    assert_ne!(agent.id, supervisor_agent.id);
    assert!(err(l.bring_back_project(&o.project, &no_providers(), OWNER)).contains("not archived"));
}

#[test]
fn a_department_is_archived_with_everything_in_it_and_comes_back_whole() {
    let w = world();
    let o = org(&w);
    let l = &w.l;
    // The website's reviewer also reviews the staff engineer's team.
    l.assign_oversight(OversightKind::Review, &o.reviewer, &o.staff, OWNER)
        .unwrap();
    // Unfinished work anywhere in it blocks archiving.
    let task = busy(l, &o.developer);
    assert!(err(l.archive_department(&o.dept, OWNER)).contains("unfinished task"));
    l.transition_task(&task, TaskState::Cancelled, OWNER, None)
        .unwrap();
    let d = l.archive_department(&o.dept, OWNER).unwrap();
    // Both sides of the assignment were archived; it ended once.
    let ended = |l: &plenipo_ledger::Ledger| {
        types(l)
            .iter()
            .filter(|t| *t == "org.oversight_ended")
            .count()
    };
    assert_eq!(ended(l), 1);
    assert!(d.archived_at.is_some());
    assert_eq!(d.status, "inactive");
    for id in [
        &o.manager,
        &o.supervisor,
        &o.developer,
        &o.reviewer,
        &o.staff,
    ] {
        assert_eq!(state(l, id), PositionState::Archived, "{id}");
    }
    let project = l.project(&o.project).unwrap().unwrap();
    assert_eq!(project.status, "archived");
    assert_eq!(project.metadata["archive"]["with"]["id"], o.dept.as_str());
    assert!(types(l).iter().any(|t| t == "org.department_archived"));
    // Its project comes back with it, not alone.
    assert!(
        err(l.bring_back_project(&o.project, &no_providers(), OWNER))
            .contains("bring the department back")
    );
    let back = l
        .bring_back_department(&o.dept, &no_providers(), OWNER)
        .unwrap();
    assert!(back.archived_at.is_none());
    assert_eq!(back.status, "active");
    assert_eq!(l.project(&o.project).unwrap().unwrap().status, "active");
    for id in [
        &o.manager,
        &o.supervisor,
        &o.developer,
        &o.reviewer,
        &o.staff,
    ] {
        assert_eq!(state(l, id), PositionState::Active, "{id}");
    }
    assert!(types(l).iter().any(|t| t == "org.department_restored"));
    assert!(
        l.org_records()
            .unwrap()
            .oversight
            .iter()
            .any(|x| x.active && x.overseer_id == o.reviewer && x.target_id == o.staff),
        "the review assignment came back"
    );
}

#[test]
fn archiving_or_deleting_goes_no_further_than_what_it_names() {
    let w = world();
    let o = org(&w);
    let l = &w.l;
    // Engineering is run by a VP, and the Development manager reports to that VP.
    let (engineering, vp) = l
        .create_department_with_head(
            "Engineering",
            "",
            &new_position(&w.roles["VP"], "VP of Engineering", None),
            OWNER,
        )
        .unwrap();
    l.move_position(&o.manager, Some(&vp.id), OWNER).unwrap();
    // Archiving Engineering would take Development's manager along: refused, with what to do.
    let why = err(l.archive_department(&engineering.id, OWNER));
    assert!(
        why.contains("Development Manager runs the Development department"),
        "{why}"
    );
    assert_eq!(state(l, &vp.id), PositionState::Active);
    assert_eq!(state(l, &o.manager), PositionState::Active);
    assert!(l
        .department(&engineering.id)
        .unwrap()
        .unwrap()
        .archived_at
        .is_none());
    // Archived one at a time, each goes on its own.
    l.archive_department(&o.dept, OWNER).unwrap();
    l.archive_department(&engineering.id, OWNER).unwrap();
    assert_eq!(state(l, &vp.id), PositionState::Archived);
    // Deleting Engineering for good would delete Development's manager and team, which were
    // archived with Development: refused.
    let why = err(l.deletion_plan("department", &engineering.id));
    assert!(
        why.contains("Development Manager runs the Development department"),
        "{why}"
    );
    assert!(
        err(l.delete_department_for_good(&engineering.id, &[], OWNER))
            .contains("delete Development for good first")
    );
    assert_eq!(state(l, &o.manager), PositionState::Archived);
    assert!(l
        .position(&o.manager)
        .unwrap()
        .unwrap()
        .deleted_at
        .is_none());
    // Development still comes back whole once its VP is back.
    let why = err(l.bring_back_department(&o.dept, &no_providers(), OWNER));
    assert!(why.contains("bring VP of Engineering back first"), "{why}");
    // Deleting Development first, then Engineering, works.
    l.delete_department_for_good(&o.dept, &[], OWNER).unwrap();
    let plan = l.deletion_plan("department", &engineering.id).unwrap();
    assert_eq!(
        plan.positions
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>(),
        [vp.id.as_str()]
    );
    l.delete_department_for_good(&engineering.id, &[], OWNER)
        .unwrap();
}

#[test]
fn a_position_deleted_for_good_takes_no_other_departments_lead() {
    let w = world();
    let o = org(&w);
    let l = &w.l;
    // A VP who runs no department, with the Development manager reporting to it.
    let (vp, _) = l
        .create_position(&new_position(&w.roles["VP"], "VP of Products", None), OWNER)
        .unwrap();
    l.move_position(&o.manager, Some(&vp.id), OWNER).unwrap();
    l.archive_department(&o.dept, OWNER).unwrap();
    l.archive_position(&vp.id, OWNER).unwrap();
    let why = err(l.delete_position_for_good(&vp.id, &[], OWNER));
    assert!(
        why.contains("Development Manager runs the Development department"),
        "{why}"
    );
    for id in [&o.manager, &o.supervisor, &o.developer, &o.staff] {
        assert!(
            l.position(id).unwrap().unwrap().deleted_at.is_none(),
            "{id}"
        );
    }
    // Brought back, Development comes back with its manager and team.
    l.bring_back_position(&vp.id, &no_providers(), OWNER)
        .unwrap();
    l.bring_back_department(&o.dept, &no_providers(), OWNER)
        .unwrap();
    for id in [&o.manager, &o.supervisor, &o.developer, &o.staff] {
        assert_eq!(state(l, id), PositionState::Active, "{id}");
    }
}

// ---- Delete for good (ADR-043) --------------------------------------------------------------

#[test]
fn deleting_for_good_leaves_a_short_record_that_still_names_it() {
    let w = world();
    let o = org(&w);
    let l = &w.l;
    let history = done(l, &o.developer);
    // Only an archived agent, and never while it has unfinished work.
    assert!(err(l.delete_position_for_good(&o.developer, &[], OWNER)).contains("archive it first"));
    l.archive_position(&o.developer, OWNER).unwrap();
    let gone = l
        .delete_position_for_good(&o.developer, &[], OWNER)
        .unwrap();
    assert_eq!(gone.positions, [o.developer.as_str()]);
    let record = l.position(&o.developer).unwrap().unwrap();
    assert!(record.is_deleted());
    assert_eq!(record.title, "Website Developer");
    assert_eq!(record.role_id, w.roles["Senior Developer"]);
    assert!(record.archived_at.is_some() && record.created_at > 0);
    assert_eq!(record.metadata["deleted"]["by"], OWNER);
    assert!(record.reports_to.is_none() && record.runtime_id.is_none());
    // Older work still points at it by ID, and it still has its name.
    let task = l.task(&history).unwrap().unwrap();
    assert_eq!(
        task.metadata["workforce"]["positionId"],
        o.developer.as_str()
    );
    let event = l
        .recent_events(50)
        .unwrap()
        .into_iter()
        .find(|e| e.event_type == "org.position_deleted")
        .unwrap();
    assert_eq!(event.payload["title"], "Website Developer");
    assert_eq!(event.payload["role"], "Senior Developer");
    // A short record never changes again, and is never removed.
    assert!(err(l.bring_back_position(&o.developer, &no_providers(), OWNER)).contains("deleted"));
    assert!(err(l.delete_position_for_good(&o.developer, &[], OWNER)).contains("already deleted"));
}

#[test]
fn deleting_for_good_is_refused_while_anything_has_unfinished_work() {
    let w = world();
    let o = org(&w);
    let l = &w.l;
    l.archive_project(&o.project, OWNER).unwrap();
    // Work that started before, still going (a task can outlive an archive in a crash).
    let task = busy(l, &o.reviewer);
    let refused = err(l.delete_project_for_good(&o.project, &[], OWNER));
    assert!(refused.contains("unfinished task"), "{refused}");
    l.transition_task(&task, TaskState::Cancelled, OWNER, None)
        .unwrap();
    let gone = l.delete_project_for_good(&o.project, &[], OWNER).unwrap();
    assert_eq!(gone.projects, [o.project.as_str()]);
    assert_eq!(gone.positions.len(), 3, "the supervisor and its team");
    let record = l.project(&o.project).unwrap().unwrap();
    assert_eq!(record.name, "Website (deleted)");
    assert!(record.local_path.is_none() && record.allowed_runtimes.is_empty());
    assert_eq!(record.metadata["deleted"]["name"], "Website");
    // The name can be used again.
    l.create_project_with_coordinator(
        &o.dept,
        &settings("Website"),
        &new_position(&w.roles["Supervisor"], "Website Supervisor", None),
        OWNER,
    )
    .unwrap();
}

// ---- Experience and the Workforce (ADR-045) ------------------------------------------------

#[test]
fn experience_counts_kept_lessons_and_finished_tasks() {
    let w = world();
    let o = org(&w);
    let l = &w.l;
    let t1 = done(l, &o.developer);
    done(l, &o.developer);
    busy(l, &o.developer);
    l.add_lessons(
        &NewLessons {
            role_id: w.roles["Senior Developer"].clone(),
            task_id: t1.clone(),
            position_id: Some(o.developer.clone()),
            worker: "Website Developer".into(),
            texts: vec!["Run the tests first.".into(), "Read the README.".into()],
            from_web: false,
            keep: true,
        },
        "plenipo",
    )
    .unwrap();
    let counts = l.experience_counts().unwrap();
    let dev = counts[&o.developer];
    assert_eq!((dev.kept_lessons, dev.tasks_done), (2, 2));
    assert!(dev.first_task_at.is_some() && dev.last_task_at.is_some());
    assert!(!counts.contains_key(&o.reviewer));
}

#[test]
fn a_saved_agent_leaves_a_short_record_and_is_hired_again_with_its_experience() {
    let w = world();
    let o = org(&w);
    let l = &w.l;
    let t = done(l, &o.developer);
    l.add_lessons(
        &NewLessons {
            role_id: w.roles["Senior Developer"].clone(),
            task_id: t,
            position_id: Some(o.developer.clone()),
            worker: "Website Developer".into(),
            texts: vec!["Run the tests first.".into()],
            from_web: false,
            keep: true,
        },
        "plenipo",
    )
    .unwrap();
    l.archive_project(&o.project, OWNER).unwrap();
    let settings = json!({ "runtimeId": "codex", "model": null, "learns": false });
    let gone = l
        .delete_project_for_good(
            &o.project,
            &[SaveAgent {
                position_id: o.developer.clone(),
                settings: settings.clone(),
            }],
            OWNER,
        )
        .unwrap();
    assert_eq!(gone.saved.len(), 1);
    assert_eq!(gone.positions.len(), 2, "the supervisor and the reviewer");
    let saved = l.saved_agents().unwrap();
    assert_eq!(saved.len(), 1);
    let s = &saved[0];
    assert_eq!(s.title, "Website Developer");
    assert_eq!(s.settings, settings);
    assert_eq!(s.experience["keptLessons"], 1);
    assert_eq!(s.experience["tasksDone"], 1);
    assert_eq!(s.experience["places"], json!(["Website", "Development"]));
    assert_eq!(s.lessons, ["Run the tests first."]);
    let record = l.position(&o.developer).unwrap().unwrap();
    assert!(record.is_deleted());
    assert_eq!(record.metadata["deleted"]["movedTo"], "workforce");
    assert!(
        err(l.bring_back_position(&o.developer, &no_providers(), OWNER))
            .contains("in your Workforce")
    );
    // Hired again into another team: its experience carries on and it leaves the Workforce.
    let (again, _) = l
        .create_position(
            &NewPosition {
                from_workforce: Some(s.id.clone()),
                ..new_position(&w.roles["Senior Developer"], &s.title, Some(&o.staff))
            },
            OWNER,
        )
        .unwrap();
    assert_eq!(again.metadata["experience"]["keptLessons"], 1);
    assert_eq!(again.metadata["experience"]["savedId"], s.id.as_str());
    assert!(l.saved_agents().unwrap().is_empty());
    // The Ledger keeps what it came back with.
    let hired = l
        .recent_events(1000)
        .unwrap()
        .into_iter()
        .find(|e| e.event_type == "org.agent_hired_from_workforce")
        .unwrap();
    assert_eq!(hired.payload["settings"], settings);
    assert_eq!(hired.payload["experience"]["tasksDone"], 1);
    // It keeps its role.
    let another = l.saved_agents().unwrap();
    assert!(another.is_empty());
}

#[test]
fn saving_on_its_own_needs_an_archived_agent_that_leads_no_one() {
    let w = world();
    let o = org(&w);
    let l = &w.l;
    let save = |id: &str| SaveAgent {
        position_id: id.into(),
        settings: json!({}),
    };
    assert!(err(l.save_to_workforce(&save(&o.staff), OWNER)).contains("archive it first"));
    l.archive_project(&o.project, OWNER).unwrap();
    assert!(err(l.save_to_workforce(&save(&o.supervisor), OWNER)).contains("leads a project"));
    let s = l.save_to_workforce(&save(&o.reviewer), OWNER).unwrap();
    assert_eq!(s.title, "Website Reviewer");
    // Deleting it from the Workforce removes it; its record stays in the events.
    l.delete_saved_agent(&s.id, OWNER).unwrap();
    assert!(l.saved_agents().unwrap().is_empty());
    assert!(types(l).iter().any(|t| t == "org.saved_agent_deleted"));
}
