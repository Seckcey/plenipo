//! Phase 18 in the Ledger: where the owner put each tile on the canvas (ADR-053), and moving an
//! oversight line's end (ADR-053 §8). Lending is tested beside its code
//! (`src/workforce/loans.rs`), since it records workers.

use plenipo_ledger::{
    Ledger, LedgerError, NewPosition, OversightKind, ProjectSettings, RoleTemplate, RoleType,
    TilePlace,
};
use serde_json::Value;

const OWNER: &str = "owner";

fn template(name: &'static str, role_type: RoleType, persistent: bool) -> RoleTemplate {
    RoleTemplate {
        name,
        description: "",
        role_type,
        persistent,
        metadata: Value::Null,
        formerly: &[],
    }
}

fn position(role: &str, title: &str, reports_to: Option<&str>) -> NewPosition {
    NewPosition {
        title: title.into(),
        role_id: role.into(),
        reports_to: reports_to.map(str::to_owned),
        staffed: true,
        ..NewPosition::default()
    }
}

/// Two projects in one department; a reviewer and a developer on the first.
struct World {
    l: Ledger,
    first_lead: String,
    second_lead: String,
    reviewer: String,
    developer: String,
    staff: String,
}

fn world(l: Ledger) -> World {
    let roles = l
        .ensure_roles(
            &[
                template("Department Manager", RoleType::DepartmentManager, true),
                template("Project Coordinator", RoleType::ProjectCoordinator, true),
                template("Code Reviewer", RoleType::Worker, false),
                template("Senior Developer", RoleType::Worker, false),
                template("Staff Engineer", RoleType::Worker, true),
            ],
            "plenipo",
        )
        .unwrap();
    let role = |n: &str| roles.iter().find(|r| r.name == n).unwrap().id.clone();
    let (dept, _) = l
        .create_department_with_head(
            "Development",
            "",
            &position(&role("Department Manager"), "Development Manager", None),
            OWNER,
        )
        .unwrap();
    let project = |name: &str| {
        l.create_project_with_coordinator(
            &dept.id,
            &ProjectSettings {
                name: name.into(),
                description: String::new(),
                allowed_runtimes: vec!["claude-code".into()],
                ..ProjectSettings::default()
            },
            &position(
                &role("Project Coordinator"),
                &format!("{name} Supervisor"),
                None,
            ),
            OWNER,
        )
        .unwrap()
        .1
    };
    let first = project("Website");
    let second = project("Shop");
    let hire = |r: &str, title: &str, lead: &str| {
        l.create_position(&position(&role(r), title, Some(lead)), OWNER)
            .unwrap()
            .0
            .id
    };
    let reviewer = hire("Code Reviewer", "Reviewer", &first.id);
    let developer = hire("Senior Developer", "Developer", &first.id);
    let staff = hire("Staff Engineer", "Staff Engineer", &first.id);
    World {
        first_lead: first.id,
        second_lead: second.id,
        reviewer,
        developer,
        staff,
        l,
    }
}

fn place(tile: &str, x: f64, y: f64) -> TilePlace {
    TilePlace {
        tile_id: tile.into(),
        x,
        y,
    }
}

fn err<T: std::fmt::Debug>(r: Result<T, LedgerError>) -> String {
    match r {
        Err(LedgerError::InvalidInput(m)) => m,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_moved_tile_stays_where_it_was_put_after_a_restart_and_tidy_up_forgets_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plenipo.db");
    let w = world(Ledger::open(&path).unwrap());
    w.l.place_tiles(&[
        place("owner", -40.0, 12.5),
        place("organization", 180.0, 0.0),
        place(&w.first_lead, 900.0, -320.0),
        place(&w.developer, 1260.0, -300.0),
    ])
    .unwrap();
    // A second drag moves one of them again; the others keep their places.
    w.l.place_tiles(&[place(&w.developer, 1300.0, -280.0)])
        .unwrap();
    let before = w.l.recent_events(100).unwrap().len();
    drop(w);

    let l = Ledger::open(&path).unwrap();
    let places = l.canvas_places().unwrap();
    assert_eq!(places.len(), 4, "the places survive a restart");
    let developer = places
        .iter()
        .find(|p| p.x == 1300.0)
        .expect("the second drag wins");
    assert_eq!(developer.y, -280.0);
    assert_eq!(
        l.recent_events(100).unwrap().len(),
        before,
        "placing tiles is not in the Activity trail"
    );

    // Tidy up forgets every place and returns them for Undo.
    let forgotten = l.tidy_up().unwrap();
    assert_eq!(forgotten.len(), 4);
    assert!(l.canvas_places().unwrap().is_empty());
    l.place_tiles(&forgotten).unwrap();
    assert_eq!(l.canvas_places().unwrap(), forgotten, "Undo puts them back");
}

#[test]
fn only_tiles_on_the_canvas_can_be_placed_and_only_on_it() {
    let w = world(Ledger::open_in_memory().unwrap());
    assert!(err(w.l.place_tiles(&[place("no-such-tile", 0.0, 0.0)]))
        .contains("not on the organization canvas"));
    assert!(err(w.l.place_tiles(&[place("owner", f64::NAN, 0.0)])).contains("on the canvas"));
    assert!(err(w.l.place_tiles(&[place("owner", 0.0, 200_000.0)])).contains("on the canvas"));
    let many: Vec<TilePlace> = (0..501).map(|_| place("owner", 0.0, 0.0)).collect();
    assert!(err(w.l.place_tiles(&many)).contains("at most 500"));
    // All or nothing: one bad tile keeps the good ones from being saved.
    assert!(w
        .l
        .place_tiles(&[place("owner", 1.0, 1.0), place("nope", 0.0, 0.0)])
        .is_err());
    assert!(w.l.canvas_places().unwrap().is_empty());
}

#[test]
fn a_tile_deleted_for_good_leaves_the_canvas() {
    let w = world(Ledger::open_in_memory().unwrap());
    w.l.place_tiles(&[
        place(&w.developer, 10.0, 10.0),
        place(&w.reviewer, 20.0, 20.0),
    ])
    .unwrap();
    // An archived tile keeps its place (it can be brought back there).
    w.l.archive_position(&w.developer, OWNER).unwrap();
    assert_eq!(w.l.canvas_places().unwrap().len(), 2);
    w.l.delete_position_for_good(&w.developer, &[], OWNER)
        .unwrap();
    let left = w.l.canvas_places().unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].tile_id, w.reviewer);
    // A short record cannot be placed again.
    assert!(w.l.place_tiles(&[place(&w.developer, 0.0, 0.0)]).is_err());
}

#[test]
fn an_oversight_line_is_moved_by_its_end_in_one_step() {
    let w = world(Ledger::open_in_memory().unwrap());
    let o =
        w.l.assign_oversight(OversightKind::Review, &w.reviewer, &w.first_lead, OWNER)
            .err();
    assert!(o.is_some(), "a team member cannot oversee its own team");
    // The developer's team is the first project; it reviews the second project's team.
    let review =
        w.l.assign_oversight(OversightKind::Review, &w.developer, &w.second_lead, OWNER)
            .unwrap();

    // The team's end: the same reviewer now oversees another team.
    let moved =
        w.l.retarget_oversight(&review.id, None, Some(&w.staff), OWNER)
            .unwrap();
    assert_eq!(moved.kind, OversightKind::Review);
    assert_eq!(moved.overseer_id, w.developer);
    assert_eq!(moved.target_id, w.staff);
    let records = w.l.org_records().unwrap();
    assert_eq!(records.oversight.len(), 1, "the old assignment ended");
    let events: Vec<String> =
        w.l.recent_events(10)
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .collect();
    let ended = events
        .iter()
        .position(|t| t == "org.oversight_ended")
        .unwrap();
    let assigned = events
        .iter()
        .position(|t| t == "org.oversight_assigned")
        .unwrap();
    assert!(
        ended > assigned,
        "ended first, then assigned (newest first)"
    );

    // The overseer's end: a full-time position cannot oversee, so nothing changes.
    let refused = err(w
        .l
        .retarget_oversight(&moved.id, Some(&w.first_lead), None, OWNER));
    assert!(refused.contains("full-time"), "{refused}");
    let still = w.l.org_records().unwrap().oversight;
    assert_eq!(still.len(), 1);
    assert_eq!(still[0].id, moved.id, "a refused move changes nothing");

    // The same place is not a move; an ended assignment cannot be moved.
    assert!(err(w
        .l
        .retarget_oversight(&moved.id, Some(&w.developer), None, OWNER))
    .contains("another agent"));
    assert!(err(w
        .l
        .retarget_oversight(&review.id, None, Some(&w.second_lead), OWNER))
    .contains("has ended"));
}
