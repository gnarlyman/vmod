use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::fs;

/// Information about a backup
#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub name: String,
    pub created_at: SystemTime,
}

/// Manages mod list backups for a profile
pub struct BackupManager {
    backup_dir: PathBuf,
}

impl BackupManager {
    /// Create a new BackupManager for the given profile
    pub fn new(profile_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Could not find config directory")?
            .join("vmod")
            .join("profiles")
            .join(profile_name)
            .join("backups");

        Ok(Self {
            backup_dir: config_dir,
        })
    }

    /// Create a backup with the given name
    pub fn create_backup(
        &self,
        name: &str,
        mod_state_path: &Path,
        sections_path: &Path,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        // Sanitize the backup name to be filesystem-safe
        let safe_name = sanitize_filename(name);
        if safe_name.is_empty() {
            return Err("Invalid backup name".into());
        }

        let backup_path = self.backup_dir.join(&safe_name);

        // Create backup directory
        fs::create_dir_all(&backup_path)?;

        // Copy mod_state.json if it exists
        if mod_state_path.exists() {
            fs::copy(mod_state_path, backup_path.join("mod_state.json"))?;
        }

        // Copy sections.json if it exists
        if sections_path.exists() {
            fs::copy(sections_path, backup_path.join("sections.json"))?;
        }

        Ok(backup_path)
    }

    /// Restore a backup to the given destinations
    pub fn restore_backup(
        &self,
        name: &str,
        mod_state_dest: &Path,
        sections_dest: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let safe_name = sanitize_filename(name);
        let backup_path = self.backup_dir.join(&safe_name);

        if !backup_path.exists() {
            return Err(format!("Backup '{}' not found", name).into());
        }

        // Ensure destination directories exist
        if let Some(parent) = mod_state_dest.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(parent) = sections_dest.parent() {
            fs::create_dir_all(parent)?;
        }

        // Restore mod_state.json
        let mod_state_backup = backup_path.join("mod_state.json");
        if mod_state_backup.exists() {
            fs::copy(&mod_state_backup, mod_state_dest)?;
        }

        // Restore sections.json
        let sections_backup = backup_path.join("sections.json");
        if sections_backup.exists() {
            fs::copy(&sections_backup, sections_dest)?;
        }

        Ok(())
    }

    /// List all backups, sorted by date (most recent first)
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>, Box<dyn std::error::Error>> {
        let mut backups = Vec::new();

        if !self.backup_dir.exists() {
            return Ok(backups);
        }

        for entry in fs::read_dir(&self.backup_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Check if this is a valid backup (has at least one config file)
                let has_mod_state = path.join("mod_state.json").exists();
                let has_sections = path.join("sections.json").exists();

                if has_mod_state || has_sections {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let created_at = entry.metadata()?.modified()?;

                    backups.push(BackupInfo {
                        name,
                        created_at,
                    });
                }
            }
        }

        // Sort by creation time, most recent first
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(backups)
    }

    /// Get a default name for a new backup
    /// Returns the most recent backup name, or a timestamped name if no backups exist
    pub fn get_default_name(&self) -> String {
        if let Ok(backups) = self.list_backups() {
            if let Some(most_recent) = backups.first() {
                return most_recent.name.clone();
            }
        }

        // Generate timestamp-based name
        let now = chrono::Local::now();
        format!("modlist_backup_{}", now.format("%Y%m%d_%H%M%S"))
    }
}

/// Sanitize a filename by removing/replacing invalid characters
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' || c == ' ' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("my backup"), "my backup");
        assert_eq!(sanitize_filename("my/backup"), "my_backup");
        assert_eq!(sanitize_filename("backup:test"), "backup_test");
        assert_eq!(sanitize_filename("  trimmed  "), "trimmed");
    }

    #[test]
    fn test_backup_manager_list_empty() {
        // Create a temp dir to use as config
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");

        let manager = BackupManager {
            backup_dir,
        };

        let backups = manager.list_backups().unwrap();
        assert!(backups.is_empty());
    }
}
