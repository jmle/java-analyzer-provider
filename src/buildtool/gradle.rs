// Gradle integration - build.gradle parsing and dependency resolution

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info, warn};

/// Represents a Gradle dependency
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GradleDependency {
    pub group: String,
    pub name: String,
    pub version: Option<String>,
    pub configuration: Option<String>, // e.g., "implementation", "testImplementation"
}

impl GradleDependency {
    /// Get the dependency identifier in the format group:name:version
    pub fn to_identifier(&self) -> String {
        if let Some(ref version) = self.version {
            format!("{}:{}:{}", self.group, self.name, version)
        } else {
            format!("{}:{}", self.group, self.name)
        }
    }

    /// Get the short name
    pub fn artifact_name(&self) -> &str {
        &self.name
    }
}

/// Gradle dependency resolver
pub struct GradleResolver {
    build_file: PathBuf,
    gradle_cmd: String,
}

impl GradleResolver {
    /// Create a new Gradle resolver for the given build file
    pub fn new(build_file: PathBuf) -> Self {
        Self {
            build_file,
            gradle_cmd: "gradle".to_string(),
        }
    }

    /// Set custom Gradle command (e.g., "./gradlew" for Gradle wrapper)
    pub fn with_gradle_cmd(mut self, cmd: String) -> Self {
        self.gradle_cmd = cmd;
        self
    }

    /// Check if Gradle is available
    pub fn is_gradle_available(&self) -> bool {
        Command::new(&self.gradle_cmd)
            .arg("--version")
            .output()
            .is_ok()
    }

    /// Resolve dependencies using gradle dependencies
    pub fn resolve_dependencies(&self) -> Result<Vec<GradleDependency>> {
        info!("Resolving Gradle dependencies for: {}", self.build_file.display());

        if !self.is_gradle_available() {
            info!("Gradle is not available, falling back to build file parsing");
            return self.parse_build_file();
        }

        info!("Gradle is available, attempting to run dependencies task");

        // Get the project directory (where build.gradle is located)
        let project_dir = self.build_file.parent().unwrap_or(Path::new("."));

        let output = Command::new(&self.gradle_cmd)
            .arg("dependencies")
            .arg("--configuration=compileClasspath")
            .current_dir(project_dir)
            .output()
            .context("Failed to execute gradle dependencies")?;

        if !output.status.success() {
            warn!("gradle dependencies failed, falling back to build file parsing");
            debug!("Gradle stderr: {}", String::from_utf8_lossy(&output.stderr));
            return self.parse_build_file();
        }

        let deps_output = String::from_utf8_lossy(&output.stdout);
        let dependencies = self.parse_dependencies_output(&deps_output);

        info!("Resolved {} dependencies (including transitive)", dependencies.len());

        // If gradle returned no dependencies, fall back to parsing
        if dependencies.is_empty() {
            info!("No dependencies found in gradle output, falling back to build file parsing");
            return self.parse_build_file();
        }

        Ok(dependencies)
    }

    /// Parse dependencies directly from build.gradle or build.gradle.kts
    fn parse_build_file(&self) -> Result<Vec<GradleDependency>> {
        info!("Parsing build file directly: {}", self.build_file.display());

        let content = std::fs::read_to_string(&self.build_file)
            .with_context(|| format!("Failed to read build file: {}", self.build_file.display()))?;

        let dependencies = self.extract_dependencies_from_source(&content);

        info!("Found {} dependencies in build file", dependencies.len());
        for dep in &dependencies {
            debug!("  - {}:{}", dep.group, dep.name);
        }

        Ok(dependencies)
    }

    /// Extract dependencies from Gradle build file source
    fn extract_dependencies_from_source(&self, content: &str) -> Vec<GradleDependency> {
        let mut dependencies = Vec::new();

        // Match patterns like:
        // implementation 'group:name:version'
        // implementation "group:name:version"
        // implementation('group:name:version')
        // implementation("group:name:version")
        // testImplementation group: 'group', name: 'name', version: 'version'

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
                continue;
            }

            // Try to parse compact format: implementation 'group:name:version'
            if let Some(dep) = self.parse_compact_dependency(trimmed) {
                dependencies.push(dep);
                continue;
            }

            // Try to parse map format: implementation group: 'group', name: 'name', version: 'version'
            if let Some(dep) = self.parse_map_dependency(trimmed) {
                dependencies.push(dep);
            }
        }

        dependencies
    }

    /// Parse compact dependency format: implementation 'group:name:version'
    fn parse_compact_dependency(&self, line: &str) -> Option<GradleDependency> {
        // Configuration keywords
        let configs = ["implementation", "api", "compileOnly", "runtimeOnly",
                       "testImplementation", "testCompileOnly", "testRuntimeOnly",
                       "annotationProcessor", "kapt"];

        for config in &configs {
            if line.contains(config) {
                // Extract the quoted string
                let parts: Vec<&str> = line.split(|c| c == '\'' || c == '"').collect();
                if parts.len() >= 2 {
                    let coord = parts[1];
                    if let Some(dep) = self.parse_coordinate(coord) {
                        let mut dep = dep;
                        dep.configuration = Some(config.to_string());
                        return Some(dep);
                    }
                }
            }
        }

        None
    }

    /// Parse map dependency format: implementation group: 'group', name: 'name', version: 'version'
    fn parse_map_dependency(&self, line: &str) -> Option<GradleDependency> {
        // This is a simplified parser - a full implementation would need proper Groovy/Kotlin parsing
        if !line.contains("group:") || !line.contains("name:") {
            return None;
        }

        let mut group = String::new();
        let mut name = String::new();
        let mut version: Option<String> = None;
        let mut configuration: Option<String> = None;

        // Extract configuration
        let configs = ["implementation", "api", "compileOnly", "runtimeOnly",
                       "testImplementation", "testCompileOnly", "testRuntimeOnly"];
        for config in &configs {
            if line.contains(config) {
                configuration = Some(config.to_string());
                break;
            }
        }

        // Very basic extraction of group, name, version
        // This is simplified and won't handle all edge cases
        let parts: Vec<&str> = line.split(|c| c == '\'' || c == '"').collect();
        for i in 0..parts.len() {
            if parts[i].contains("group:") && i + 1 < parts.len() {
                group = parts[i + 1].to_string();
            } else if parts[i].contains("name:") && i + 1 < parts.len() {
                name = parts[i + 1].to_string();
            } else if parts[i].contains("version:") && i + 1 < parts.len() {
                version = Some(parts[i + 1].to_string());
            }
        }

        if !group.is_empty() && !name.is_empty() {
            Some(GradleDependency {
                group,
                name,
                version,
                configuration,
            })
        } else {
            None
        }
    }

    /// Parse coordinate string (group:name:version)
    fn parse_coordinate(&self, coord: &str) -> Option<GradleDependency> {
        let parts: Vec<&str> = coord.split(':').collect();
        if parts.len() >= 2 {
            let group = parts[0].to_string();
            let name = parts[1].to_string();
            let version = if parts.len() >= 3 {
                Some(parts[2].to_string())
            } else {
                None
            };

            Some(GradleDependency {
                group,
                name,
                version,
                configuration: None,
            })
        } else {
            None
        }
    }

    /// Parse the output of gradle dependencies
    fn parse_dependencies_output(&self, output: &str) -> Vec<GradleDependency> {
        let mut dependencies = Vec::new();

        for line in output.lines() {
            // Look for dependency lines like:
            // +--- group:name:version
            // \--- group:name:version
            // |    +--- group:name:version
            if !line.contains("---") {
                continue;
            }

            // Extract the dependency coordinate
            let parts: Vec<&str> = line.split_whitespace().collect();
            for part in parts {
                if part.contains(':') && !part.starts_with('+') && !part.starts_with('\\') {
                    if let Some(dep) = self.parse_coordinate(part) {
                        dependencies.push(dep);
                    }
                    break;
                }
            }
        }

        dependencies
    }
}

/// Find all build.gradle and build.gradle.kts files in a directory tree
pub fn find_gradle_files(path: &Path) -> Result<Vec<PathBuf>> {
    let mut gradle_files = Vec::new();

    if path.is_file() {
        let file_name = path.file_name().and_then(|n| n.to_str());
        if file_name == Some("build.gradle") || file_name == Some("build.gradle.kts") {
            gradle_files.push(path.to_path_buf());
        }
    } else if path.is_dir() {
        for entry in std::fs::read_dir(path)
            .with_context(|| format!("Failed to read directory: {}", path.display()))?
        {
            let entry = entry?;
            let entry_path = entry.path();

            // Skip build and .gradle directories
            if entry_path.is_dir() {
                if let Some(name) = entry_path.file_name() {
                    if name == "build" || name == ".gradle" {
                        continue;
                    }
                }
            }

            gradle_files.extend(find_gradle_files(&entry_path)?);
        }
    }

    Ok(gradle_files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_compact_dependency() {
        let resolver = GradleResolver::new(PathBuf::from("build.gradle"));

        let line = "    implementation 'junit:junit:4.13.2'";
        let dep = resolver.parse_compact_dependency(line).unwrap();

        assert_eq!(dep.group, "junit");
        assert_eq!(dep.name, "junit");
        assert_eq!(dep.version, Some("4.13.2".to_string()));
        assert_eq!(dep.configuration, Some("implementation".to_string()));
    }

    #[test]
    fn test_parse_coordinate() {
        let resolver = GradleResolver::new(PathBuf::from("build.gradle"));

        let coord = "org.springframework.boot:spring-boot-starter-web:2.7.0";
        let dep = resolver.parse_coordinate(coord).unwrap();

        assert_eq!(dep.group, "org.springframework.boot");
        assert_eq!(dep.name, "spring-boot-starter-web");
        assert_eq!(dep.version, Some("2.7.0".to_string()));
    }

    #[test]
    fn test_dependency_to_identifier() {
        let dep = GradleDependency {
            group: "junit".to_string(),
            name: "junit".to_string(),
            version: Some("4.13.2".to_string()),
            configuration: Some("testImplementation".to_string()),
        };

        assert_eq!(dep.to_identifier(), "junit:junit:4.13.2");
        assert_eq!(dep.artifact_name(), "junit");
    }

    #[test]
    fn test_extract_dependencies_from_source() {
        let resolver = GradleResolver::new(PathBuf::from("build.gradle"));

        let content = r#"
dependencies {
    implementation 'org.springframework.boot:spring-boot-starter-web:2.7.0'
    testImplementation 'junit:junit:4.13.2'
    compileOnly "org.projectlombok:lombok:1.18.24"
}
"#;

        let deps = resolver.extract_dependencies_from_source(content);

        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].group, "org.springframework.boot");
        assert_eq!(deps[1].group, "junit");
        assert_eq!(deps[2].group, "org.projectlombok");
    }
}
