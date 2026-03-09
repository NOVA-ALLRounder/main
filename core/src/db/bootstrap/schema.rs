#[path = "schema/repairs.rs"]
mod repairs;
#[path = "schema/tables.rs"]
mod tables;

pub(super) fn run_post_init_repairs(conn: &rusqlite::Connection) {
    repairs::run_post_init_repairs(conn);
}

pub(super) fn apply_base_schema(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    tables::apply_base_schema(conn)
}
