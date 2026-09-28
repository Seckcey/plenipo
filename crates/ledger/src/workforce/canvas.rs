//! Where the owner put each tile on the organization canvas (Phase 18, ADR-053 §3–§5). These are
//! how the owner looks at the organization, not something that happened in it, so placing tiles
//! records no event; the places go with the Ledger's backups and exports.

use rusqlite::{params, Connection};

use super::{all, invalid, now};
use crate::dto::TilePlace;
use crate::error::Result;
use crate::Ledger;

/// The owner's tile and the organization's tile.
pub const OWNER_TILE: &str = "owner";
pub const ORGANIZATION_TILE: &str = "organization";
/// How many tiles one call may place (a team moved at once).
pub const MAX_PLACES: usize = 500;
/// How far from the middle of the canvas a tile may go.
pub const MAX_COORDINATE: f64 = 100_000.0;

fn place_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TilePlace> {
    Ok(TilePlace {
        tile_id: r.get(0)?,
        x: r.get(1)?,
        y: r.get(2)?,
    })
}

fn every_place(c: &Connection) -> Result<Vec<TilePlace>> {
    all(
        c,
        "SELECT tile_id, x, y FROM canvas_places ORDER BY tile_id",
        [],
        place_row,
    )
}

/// Forget a tile's place (a position deleted for good, or moved to the Workforce).
pub(super) fn forget(c: &Connection, tile_id: &str) -> Result<()> {
    c.execute("DELETE FROM canvas_places WHERE tile_id = ?1", [tile_id])?;
    Ok(())
}

fn check(c: &Connection, place: &TilePlace) -> Result<()> {
    if !place.x.is_finite()
        || !place.y.is_finite()
        || place.x.abs() > MAX_COORDINATE
        || place.y.abs() > MAX_COORDINATE
    {
        return Err(invalid("a tile's place must be on the canvas"));
    }
    if place.tile_id == OWNER_TILE || place.tile_id == ORGANIZATION_TILE {
        return Ok(());
    }
    // A position that is on the chart or archived (it may be brought back), not a short record.
    let known: bool = c.query_row(
        "SELECT EXISTS (SELECT 1 FROM positions WHERE id = ?1 AND deleted_at IS NULL)",
        [&place.tile_id],
        |r| r.get(0),
    )?;
    if known {
        Ok(())
    } else {
        Err(invalid("that tile is not on the organization canvas"))
    }
}

impl Ledger {
    /// Every tile the owner placed by hand.
    pub fn canvas_places(&self) -> Result<Vec<TilePlace>> {
        self.read(every_place)
    }

    /// Save where the owner put `places` (one drag can move a whole team), all or nothing.
    pub fn place_tiles(&self, places: &[TilePlace]) -> Result<()> {
        if places.is_empty() {
            return Ok(());
        }
        if places.len() > MAX_PLACES {
            return Err(invalid(format!(
                "at most {MAX_PLACES} tiles can be placed at once"
            )));
        }
        self.write(|tx, _out| {
            let at = now();
            for place in places {
                check(tx, place)?;
                tx.execute(
                    "INSERT INTO canvas_places (tile_id, x, y, updated_at) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT (tile_id) DO UPDATE SET x = excluded.x, y = excluded.y,
                         updated_at = excluded.updated_at",
                    params![place.tile_id, place.x, place.y, at],
                )?;
            }
            Ok(())
        })
    }

    /// Tidy up: forget every place, so the automatic layout comes back. Returns what it forgot,
    /// for Undo.
    pub fn tidy_up(&self) -> Result<Vec<TilePlace>> {
        self.write(|tx, _out| {
            let before = every_place(tx)?;
            tx.execute("DELETE FROM canvas_places", [])?;
            Ok(before)
        })
    }
}
