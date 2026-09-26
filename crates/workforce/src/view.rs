//! A read-only view of the organization's records: who reports to whom, departments and
//! projects by the tree, and each lead's team.

use std::collections::{HashMap, HashSet};

use plenipo_ledger::{
    Department, OrgRecords, Oversight, Position, PositionState, Project, Role, RoleType,
};

/// A member of a lead's team: an on-demand position reporting to the lead, or one assigned to
/// oversee the lead's team.
#[derive(Debug, Clone, Copy)]
pub struct TeamMember<'a> {
    pub position: &'a Position,
    /// Set when the member serves the team through an oversight assignment.
    pub oversight: Option<&'a Oversight>,
}

pub struct OrgView<'a> {
    pub records: &'a OrgRecords,
    positions: HashMap<&'a str, &'a Position>,
    roles: HashMap<&'a str, &'a Role>,
    heads: HashMap<&'a str, &'a Department>,
    coordinators: HashMap<&'a str, &'a Project>,
}

impl<'a> OrgView<'a> {
    pub fn new(records: &'a OrgRecords) -> Self {
        Self {
            positions: records
                .positions
                .iter()
                .map(|p| (p.id.as_str(), p))
                .collect(),
            roles: records.roles.iter().map(|r| (r.id.as_str(), r)).collect(),
            heads: records
                .departments
                .iter()
                .filter_map(|d| d.head_position_id.as_deref().map(|h| (h, d)))
                .collect(),
            coordinators: records
                .projects
                .iter()
                .filter_map(|p| p.coordinator_position_id.as_deref().map(|c| (c, p)))
                .collect(),
            records,
        }
    }

    pub fn position(&self, id: &str) -> Option<&'a Position> {
        self.positions.get(id).copied()
    }

    /// An active (not archived) position.
    pub fn active(&self, id: &str) -> Option<&'a Position> {
        self.position(id)
            .filter(|p| p.state == PositionState::Active)
    }

    pub fn role(&self, p: &Position) -> Option<&'a Role> {
        self.roles.get(p.role_id.as_str()).copied()
    }

    pub fn persistent(&self, p: &Position) -> bool {
        self.role(p).is_some_and(|r| r.persistent)
    }

    pub fn kind(&self, p: &Position) -> RoleType {
        self.role(p).map_or(RoleType::Worker, |r| r.role_type)
    }

    /// The department `id` heads.
    pub fn heads(&self, id: &str) -> Option<&'a Department> {
        self.heads.get(id).copied()
    }

    /// The project `id` coordinates.
    pub fn coordinates(&self, id: &str) -> Option<&'a Project> {
        self.coordinators.get(id).copied()
    }

    /// `id` and its supervisors, nearest first (stops at a cycle, which the Ledger prevents).
    fn chain(&self, id: &str) -> Vec<&'a Position> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut current = self.position(id);
        while let Some(p) = current {
            if !seen.insert(p.id.as_str()) {
                break;
            }
            out.push(p);
            current = p.reports_to.as_deref().and_then(|s| self.position(s));
        }
        out
    }

    /// The department of `id`: the nearest department head at or above it.
    pub fn department_of(&self, id: &str) -> Option<&'a Department> {
        self.chain(id).into_iter().find_map(|p| self.heads(&p.id))
    }

    /// The project of `id`: the nearest coordinator at or above it.
    pub fn project_of(&self, id: &str) -> Option<&'a Project> {
        self.chain(id)
            .into_iter()
            .find_map(|p| self.coordinates(&p.id))
    }

    /// Active positions reporting to `lead` (`None`: the owner), in display order.
    pub fn reports(&self, lead: Option<&str>) -> Vec<&'a Position> {
        let mut out: Vec<&Position> = self
            .records
            .positions
            .iter()
            .filter(|p| p.state == PositionState::Active && p.reports_to.as_deref() == lead)
            .collect();
        out.sort_by(|a, b| {
            (a.sort_key, a.created_at, &a.id).cmp(&(b.sort_key, b.created_at, &b.id))
        });
        out
    }

    /// The lead whose team `id` works in: itself when persistent, otherwise its supervisor.
    pub fn lead_of(&self, id: &str) -> Option<&'a Position> {
        let p = self.position(id)?;
        if self.persistent(p) {
            Some(p)
        } else {
            p.reports_to.as_deref().and_then(|s| self.active(s))
        }
    }

    /// A lead's team: its active on-demand reports, then the active on-demand positions
    /// assigned to oversee it.
    pub fn team(&self, lead: &str) -> Vec<TeamMember<'a>> {
        let mut out: Vec<TeamMember<'a>> = self
            .reports(Some(lead))
            .into_iter()
            .filter(|p| !self.persistent(p))
            .map(|position| TeamMember {
                position,
                oversight: None,
            })
            .collect();
        for o in self
            .records
            .oversight
            .iter()
            .filter(|o| o.active && o.target_id == lead)
        {
            if let Some(position) = self.active(&o.overseer_id) {
                if !self.persistent(position) && !out.iter().any(|m| m.position.id == position.id) {
                    out.push(TeamMember {
                        position,
                        oversight: Some(o),
                    });
                }
            }
        }
        out
    }

    /// Active positions in tree order (depth-first from the owner).
    pub fn tree_order(&self) -> Vec<&'a Position> {
        let mut out = Vec::new();
        let mut stack: Vec<&Position> = self.reports(None).into_iter().rev().collect();
        let mut seen = HashSet::new();
        while let Some(p) = stack.pop() {
            if !seen.insert(p.id.as_str()) {
                continue;
            }
            out.push(p);
            stack.extend(self.reports(Some(&p.id)).into_iter().rev());
        }
        out
    }
}
