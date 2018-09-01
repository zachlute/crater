use errors::*;
use rusqlite::Connection;
use std::collections::HashSet;

fn migrations() -> Vec<(&'static str, &'static str)> {
    let mut migrations = Vec::new();

    migrations.push((
        "initial",
        "
        CREATE TABLE experiments (
            name TEXT PRIMARY KEY,
            mode TEXT NOT NULL,
            cap_lints TEXT NOT NULL,

            toolchain_start TEXT NOT NULL,
            toolchain_end TEXT NOT NULL,

            priority INTEGER NOT NULL,
            created_at DATETIME NOT NULL,
            status TEXT NOT NULL,
            github_issue TEXT,
            github_issue_url TEXT,
            github_issue_number INTEGER,
            assigned_to TEXT,

            FOREIGN KEY (assigned_to) REFERENCES agents(name) ON DELETE SET NULL
        );

        CREATE TABLE experiment_crates (
            experiment TEXT NOT NULL,
            crate TEXT NOT NULL,

            FOREIGN KEY (experiment) REFERENCES experiments(name) ON DELETE CASCADE
        );

        CREATE TABLE results (
            experiment TEXT NOT NULL,
            crate TEXT NOT NULL,
            toolchain TEXT NOT NULL,
            result TEXT NOT NULL,
            log BLOB NOT NULL,

            PRIMARY KEY (experiment, crate, toolchain) ON CONFLICT REPLACE,
            FOREIGN KEY (experiment) REFERENCES experiments(name) ON DELETE CASCADE
        );

        CREATE TABLE shas (
            experiment TEXT NOT NULL,
            org TEXT NOT NULL,
            name TEXT NOT NULL,
            sha TEXT NOT NULL,

            FOREIGN KEY (experiment) REFERENCES experiments(name) ON DELETE CASCADE
        );

        CREATE TABLE agents (
            name TEXT PRIMARY KEY,
            last_heartbeat DATETIME
        );

        CREATE TABLE saved_names (
            issue INTEGER PRIMARY KEY ON CONFLICT REPLACE,
            experiment TEXT NOT NULL
        );
        ",
    ));

    migrations.push((
        "store_agents_revision",
        "
        ALTER TABLE agents ADD COLUMN git_revision TEXT;
        ",
    ));

    migrations.push((
        "store_skipped_crates",
        "
        ALTER TABLE experiment_crates ADD COLUMN skipped INTEGER NOT NULL DEFAULT 0;
        ",
    ));

    migrations.push((
        "add_ui_progress_percent_indexes",
        "
        CREATE INDEX experiment_crates__experiment_skipped
        ON experiment_crates (experiment, skipped);

        CREATE INDEX results__experiment
        ON results (experiment);
        ",
    ));

    migrations.push((
        "add_more_experiment_dates",
        "
        ALTER TABLE experiments ADD COLUMN started_at DATETIME;
        ALTER TABLE experiments ADD COLUMN completed_at DATETIME;
        ",
    ));

    migrations.push((
        "store_report_url",
        "
        ALTER TABLE experiments ADD COLUMN report_url TEXT;
        ",
    ));

    migrations
}

pub fn execute(db: &mut Connection) -> Result<()> {
    // If the database version is 0, create the migrations table and bump it
    let version: i32 = db.query_row("PRAGMA user_version;", &[], |r| r.get(0))?;
    if version == 0 {
        db.execute("CREATE TABLE migrations (name TEXT PRIMARY KEY);", &[])?;
        db.execute("PRAGMA user_version = 1;", &[])?;
    }

    let executed_migrations = {
        let mut prepared = db.prepare("SELECT name FROM migrations;")?;
        let mut result = HashSet::new();
        for value in prepared.query_map(&[], |row| -> String { row.get("name") })? {
            result.insert(value?);
        }

        result
    };

    for &(name, sql) in &migrations() {
        if !executed_migrations.contains(&name.to_string()) {
            let t = db.transaction()?;
            t.execute_batch(sql)
                .chain_err(|| format!("error running migration: {}", name))?;
            t.execute("INSERT INTO migrations (name) VALUES (?1)", &[&name])?;
            t.commit()?;

            info!("executed migration: {}", name);
        }
    }

    Ok(())
}
