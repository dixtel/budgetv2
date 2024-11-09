use core::panic;
use std::{any::Any, borrow::BorrowMut, fs, path::PathBuf};

use sqlx::{Execute, Pool, Postgres};
pub async fn migrate(p: &Pool<Postgres>) {
    let mut migration_files: Vec<(usize, PathBuf)> = Vec::new();
    for entry in fs::read_dir("./migrations").unwrap() {
        let entry = entry.unwrap();

        if !entry.metadata().unwrap().is_file() {
            continue;
        }

        if entry.path().extension().unwrap() != "sql" {
            continue;
        }

        let path = &entry.path();
        let (migration_number, _) = path.file_name().unwrap().to_str().unwrap().split_at(3);
        let migration_number: usize = migration_number.parse().unwrap();

        migration_files.push((migration_number, entry.path()))
    }

    migration_files.sort_by_key(|v| v.0);

    println!("starting migrationg");
    for (_, dir) in &migration_files {
        let content = fs::read_to_string(dir).unwrap();
        let path = dir.to_str().unwrap();
        println!("migrating {}", path);
        let res = sqlx::query(&content).execute(p).await.unwrap();
    }

    println!("migration end");
}
