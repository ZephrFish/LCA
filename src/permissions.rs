use std::io::{self, Write};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub enum PermissionMode {
    /// Ask for permission every time
    Ask,
    /// Allow all operations without prompting
    AllowAll,
}

#[derive(Debug, Clone)]
pub struct PermissionManager {
    mode: Arc<Mutex<PermissionMode>>,
}

impl PermissionManager {
    pub fn new(mode: PermissionMode) -> Self {
        Self {
            mode: Arc::new(Mutex::new(mode)),
        }
    }

    /// Request permission for a file write operation
    pub fn request_file_write(&self, path: &str, content_preview: &str) -> bool {
        let current_mode = self.mode.lock().unwrap().clone();

        match current_mode {
            PermissionMode::AllowAll => true,
            PermissionMode::Ask => self.prompt_user_file_write(path, content_preview),
        }
    }

    /// Request permission for a shell command execution
    pub fn request_shell_execution(&self, command: &str) -> bool {
        let current_mode = self.mode.lock().unwrap().clone();

        match current_mode {
            PermissionMode::AllowAll => true,
            PermissionMode::Ask => self.prompt_user_shell_execution(command),
        }
    }

    fn prompt_user_file_write(&self, path: &str, content_preview: &str) -> bool {
        println!("\n┌─────────────────────────────────────────────────────────────┐");
        println!("│ FILE WRITE PERMISSION REQUESTED                            │");
        println!("└─────────────────────────────────────────────────────────────┘");
        println!("  Path: {}", path);
        println!("\n  Content preview (first 200 chars):");
        println!("  ┌─────────────────────────────────────────────────────────┐");

        let preview = if content_preview.len() > 200 {
            format!("{}...", &content_preview[..200])
        } else {
            content_preview.to_string()
        };

        for line in preview.lines().take(10) {
            println!("  │ {:<57} │", line);
        }
        println!("  └─────────────────────────────────────────────────────────┘");

        println!("\n  Options:");
        println!("    [y] Allow this write");
        println!("    [n] Deny this write");
        println!("    [a] Allow ALL future operations (blanket permission)");
        println!("    [q] Quit/Cancel task");

        loop {
            print!("\n  Your choice [y/n/a/q]: ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                return false;
            }

            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => {
                    println!("  >> Write allowed\n");
                    return true;
                }
                "n" | "no" => {
                    println!("  >> Write denied\n");
                    return false;
                }
                "a" | "all" => {
                    println!("  >> WARNING: Enabling blanket permissions for this session...");
                    *self.mode.lock().unwrap() = PermissionMode::AllowAll;
                    println!("  >> All future operations will be allowed\n");
                    return true;
                }
                "q" | "quit" => {
                    println!("  >> Task cancelled\n");
                    return false;
                }
                _ => {
                    println!("  Invalid choice. Please enter y, n, a, or q.");
                }
            }
        }
    }

    fn prompt_user_shell_execution(&self, command: &str) -> bool {
        println!("\n┌─────────────────────────────────────────────────────────────┐");
        println!("│ SHELL COMMAND PERMISSION REQUESTED                         │");
        println!("└─────────────────────────────────────────────────────────────┘");
        println!("  Command: {}", command);

        println!("\n  Options:");
        println!("    [y] Execute this command");
        println!("    [n] Deny execution");
        println!("    [a] Allow ALL future operations (blanket permission)");
        println!("    [q] Quit/Cancel task");

        loop {
            print!("\n  Your choice [y/n/a/q]: ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                return false;
            }

            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => {
                    println!("  >> Execution allowed\n");
                    return true;
                }
                "n" | "no" => {
                    println!("  >> Execution denied\n");
                    return false;
                }
                "a" | "all" => {
                    println!("  >> WARNING: Enabling blanket permissions for this session...");
                    *self.mode.lock().unwrap() = PermissionMode::AllowAll;
                    println!("  >> All future operations will be allowed\n");
                    return true;
                }
                "q" | "quit" => {
                    println!("  >> Task cancelled\n");
                    return false;
                }
                _ => {
                    println!("  Invalid choice. Please enter y, n, a, or q.");
                }
            }
        }
    }

    /// Check if currently in AllowAll mode
    #[allow(dead_code)]
    pub fn is_allow_all(&self) -> bool {
        matches!(*self.mode.lock().unwrap(), PermissionMode::AllowAll)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_all_mode() {
        let pm = PermissionManager::new(PermissionMode::AllowAll);
        assert!(pm.request_file_write("/tmp/test.txt", "content"));
        assert!(pm.request_shell_execution("ls -la"));
    }

    #[test]
    fn test_is_allow_all() {
        let pm = PermissionManager::new(PermissionMode::AllowAll);
        assert!(pm.is_allow_all());

        let pm2 = PermissionManager::new(PermissionMode::Ask);
        assert!(!pm2.is_allow_all());
    }
}
