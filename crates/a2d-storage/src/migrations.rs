//! Numbered, immutable migrations (TODO 3.1). Once a migration ships, its SQL file MUST NOT
//! change — fix forward with a new, higher-numbered migration.

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

pub static MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "page_generated_pdf_asset",
        sql: include_str!("migrations/0002_page_generated_pdf_asset.sql"),
    },
    Migration {
        version: 3,
        name: "milestone6_notebook_workflows",
        sql: include_str!("migrations/0003_milestone6_notebook_workflows.sql"),
    },
    Migration {
        version: 4,
        name: "scan_registration_invariants",
        sql: include_str!("migrations/0004_scan_registration_invariants.sql"),
    },
];
