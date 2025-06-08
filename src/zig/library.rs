use std::path::{Path, PathBuf};
use tokio::fs;

/// Gets the main file based on the following possible places:
///
/// 1. The root file is `root.zig`
/// 2. The root file is the same as the project name
/// 3. There is likely one file and it's named some other way
pub async fn get_root_file(project_path: &Path) -> Option<PathBuf> {
    // root.zig
    let root_zig = project_path.join("src").join("root.zig");
    if root_zig.exists() {
        return Some(root_zig);
    }

    // {project_dir_name}.zig
    if let Some(dir_name) = project_path.file_name().and_then(|os| os.to_str()) {
        let candidate = project_path.join("src").join(format!("{}.zig", dir_name));
        if candidate.exists() {
            return Some(candidate);
        }
    }

    if let Ok(mut entries) = fs::read_dir(project_path.join("src")).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("zig") {
                return Some(path);
            }
        }
    }

    // If we reach here, no `.zig` file was found.
    None
}
