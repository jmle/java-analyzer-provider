// Build tool detection

use std::path::Path;

/// Detected build tool type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildTool {
    Maven,
    Gradle,
    Unknown,
}

/// Detect the build tool used in a project
pub fn detect_build_tool(path: &Path) -> BuildTool {
    // Check for Maven
    if path.join("pom.xml").exists() {
        return BuildTool::Maven;
    }

    // Check for Gradle
    if path.join("build.gradle").exists() || path.join("build.gradle.kts").exists() {
        return BuildTool::Gradle;
    }

    // Check if path is a pom.xml file directly
    if path.is_file() && path.file_name().map_or(false, |name| name == "pom.xml") {
        return BuildTool::Maven;
    }

    // Check if path is a build.gradle file directly
    if path.is_file() && path.file_name().map_or(false, |name| {
        name == "build.gradle" || name == "build.gradle.kts"
    }) {
        return BuildTool::Gradle;
    }

    BuildTool::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_maven() {
        let temp_dir = TempDir::new().unwrap();
        let pom_path = temp_dir.path().join("pom.xml");
        fs::write(&pom_path, "<project></project>").unwrap();

        assert_eq!(detect_build_tool(temp_dir.path()), BuildTool::Maven);
        assert_eq!(detect_build_tool(&pom_path), BuildTool::Maven);
    }

    #[test]
    fn test_detect_gradle() {
        let temp_dir = TempDir::new().unwrap();
        let build_path = temp_dir.path().join("build.gradle");
        fs::write(&build_path, "").unwrap();

        assert_eq!(detect_build_tool(temp_dir.path()), BuildTool::Gradle);
        assert_eq!(detect_build_tool(&build_path), BuildTool::Gradle);
    }

    #[test]
    fn test_detect_unknown() {
        let temp_dir = TempDir::new().unwrap();
        assert_eq!(detect_build_tool(temp_dir.path()), BuildTool::Unknown);
    }
}
