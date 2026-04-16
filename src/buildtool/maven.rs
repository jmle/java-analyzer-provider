// Maven integration - pom.xml parsing and dependency resolution

use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info, warn};

/// Represents a Maven dependency
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MavenDependency {
    pub group_id: String,
    pub artifact_id: String,
    pub version: Option<String>,
    pub scope: Option<String>,
    pub classifier: Option<String>,
    pub type_: Option<String>,
    pub optional: bool,
}

impl MavenDependency {
    /// Get the dependency identifier in the format groupId:artifactId:version
    pub fn to_identifier(&self) -> String {
        if let Some(ref version) = self.version {
            format!("{}:{}:{}", self.group_id, self.artifact_id, version)
        } else {
            format!("{}:{}", self.group_id, self.artifact_id)
        }
    }

    /// Get the dependency name in groupId.artifactId format (used by analyzer-lsp for matching)
    pub fn name(&self) -> String {
        format!("{}.{}", self.group_id, self.artifact_id)
    }
}

/// Represents a Maven POM file
#[derive(Debug, Clone)]
pub struct MavenPom {
    pub path: PathBuf,
    pub group_id: Option<String>,
    pub artifact_id: Option<String>,
    pub version: Option<String>,
    pub packaging: Option<String>,
    pub parent: Option<ParentInfo>,
    pub dependencies: Vec<MavenDependency>,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ParentInfo {
    pub group_id: String,
    pub artifact_id: String,
    pub version: String,
}

impl MavenPom {
    /// Parse a pom.xml file
    pub fn parse(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read pom.xml: {}", path.display()))?;

        Self::parse_from_string(&content, path.to_path_buf())
    }

    /// Parse from XML string
    pub fn parse_from_string(xml: &str, path: PathBuf) -> Result<Self> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut pom = MavenPom {
            path,
            group_id: None,
            artifact_id: None,
            version: None,
            packaging: None,
            parent: None,
            dependencies: Vec::new(),
            properties: HashMap::new(),
        };

        let mut buf = Vec::new();
        let mut current_path = Vec::new();
        let mut current_text = String::new();

        // Temporary storage for dependency being parsed
        let mut current_dep: Option<MavenDependency> = None;
        let mut in_dependencies = false;
        let mut in_parent = false;
        let mut parent_group_id: Option<String> = None;
        let mut parent_artifact_id: Option<String> = None;
        let mut parent_version: Option<String> = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let tag_name = std::str::from_utf8(e.name().as_ref())
                        .unwrap_or("")
                        .to_string();
                    current_path.push(tag_name.clone());

                    // Check if we're entering dependencies section
                    if tag_name == "dependencies" && current_path.len() == 2 {
                        in_dependencies = true;
                    }

                    // Check if we're entering parent section
                    if tag_name == "parent" && current_path.len() == 2 {
                        in_parent = true;
                    }

                    // Start of a new dependency
                    if tag_name == "dependency" && in_dependencies {
                        current_dep = Some(MavenDependency {
                            group_id: String::new(),
                            artifact_id: String::new(),
                            version: None,
                            scope: None,
                            classifier: None,
                            type_: None,
                            optional: false,
                        });
                    }

                    current_text.clear();
                }
                Ok(Event::End(e)) => {
                    let tag_name = std::str::from_utf8(e.name().as_ref()).unwrap_or("").to_string();

                    // Process accumulated text
                    let text = current_text.trim().to_string();

                    if !text.is_empty() {
                        let path_str: Vec<&str> = current_path.iter().map(|s| s.as_str()).collect();

                        // Project-level fields
                        if path_str.len() == 2 && path_str[1] == "groupId" && current_dep.is_none() && !in_dependencies && !in_parent {
                            pom.group_id = Some(text.clone());
                        } else if path_str.len() == 2 && path_str[1] == "artifactId" && current_dep.is_none() && !in_dependencies && !in_parent {
                            pom.artifact_id = Some(text.clone());
                        } else if path_str.len() == 2 && path_str[1] == "version" && current_dep.is_none() && !in_dependencies && !in_parent {
                            pom.version = Some(text.clone());
                        } else if path_str.len() == 2 && path_str[1] == "packaging" {
                            pom.packaging = Some(text.clone());
                        }

                        // Parent fields
                        else if path_str.len() == 3 && path_str[1] == "parent" && path_str[2] == "groupId" {
                            parent_group_id = Some(text.clone());
                        } else if path_str.len() == 3 && path_str[1] == "parent" && path_str[2] == "artifactId" {
                            parent_artifact_id = Some(text.clone());
                        } else if path_str.len() == 3 && path_str[1] == "parent" && path_str[2] == "version" {
                            parent_version = Some(text.clone());
                        }

                        // Dependency fields
                        else if let Some(dep) = &mut current_dep {
                            if path_str.len() == 4 && path_str[1] == "dependencies" && path_str[2] == "dependency" {
                                match path_str[3] {
                                    "groupId" => dep.group_id = text.clone(),
                                    "artifactId" => dep.artifact_id = text.clone(),
                                    "version" => dep.version = Some(text.clone()),
                                    "scope" => dep.scope = Some(text.clone()),
                                    "classifier" => dep.classifier = Some(text.clone()),
                                    "type" => dep.type_ = Some(text.clone()),
                                    "optional" => dep.optional = text == "true",
                                    _ => {}
                                }
                            }
                        }

                        // Properties
                        else if path_str.len() == 3 && path_str[1] == "properties" {
                            pom.properties.insert(path_str[2].to_string(), text.clone());
                        }
                    }

                    // End of dependency - add it to the list
                    if tag_name == "dependency" && current_dep.is_some() {
                        let dep = current_dep.take().unwrap();
                        if !dep.group_id.is_empty() && !dep.artifact_id.is_empty() {
                            pom.dependencies.push(dep);
                        }
                    }

                    // End of dependencies section
                    if tag_name == "dependencies" {
                        in_dependencies = false;
                    }

                    // End of parent section
                    if tag_name == "parent" {
                        in_parent = false;
                        if let (Some(gid), Some(aid), Some(ver)) =
                            (parent_group_id.take(), parent_artifact_id.take(), parent_version.take())
                        {
                            pom.parent = Some(ParentInfo {
                                group_id: gid,
                                artifact_id: aid,
                                version: ver,
                            });
                        }
                    }

                    current_path.pop();
                    current_text.clear();
                }
                Ok(Event::Text(e)) => {
                    current_text.push_str(&e.unescape().unwrap_or_default());
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    warn!("Error parsing pom.xml at position {}: {:?}", reader.buffer_position(), e);
                    break;
                }
                _ => {}
            }

            buf.clear();
        }

        Ok(pom)
    }

    /// Resolve properties in a version string
    pub fn resolve_version(&self, version: &str) -> String {
        if !version.starts_with("${") || !version.ends_with("}") {
            return version.to_string();
        }

        let prop_name = &version[2..version.len() - 1];
        self.properties
            .get(prop_name)
            .cloned()
            .unwrap_or_else(|| version.to_string())
    }
}

/// Maven dependency resolver
pub struct MavenResolver {
    pom_path: PathBuf,
    maven_cmd: String,
}

impl MavenResolver {
    /// Create a new Maven resolver for the given pom.xml
    pub fn new(pom_path: PathBuf) -> Self {
        Self {
            pom_path,
            maven_cmd: "mvn".to_string(),
        }
    }

    /// Set custom Maven command (e.g., "mvnw" for Maven wrapper)
    pub fn with_maven_cmd(mut self, cmd: String) -> Self {
        self.maven_cmd = cmd;
        self
    }

    /// Check if Maven is available
    pub fn is_maven_available(&self) -> bool {
        Command::new(&self.maven_cmd)
            .arg("--version")
            .output()
            .is_ok()
    }

    /// Resolve dependencies using mvn dependency:tree
    pub fn resolve_dependencies(&self) -> Result<Vec<MavenDependency>> {
        info!("Resolving Maven dependencies for: {}", self.pom_path.display());

        if !self.is_maven_available() {
            info!("Maven is not available, falling back to pom.xml parsing only");
            return self.parse_pom_dependencies();
        }

        info!("Maven is available, attempting to run dependency:tree");

        let output = Command::new(&self.maven_cmd)
            .arg("dependency:tree")
            .arg("-DoutputType=text")
            .arg("-DoutputFile=-") // Output to stdout
            .arg("-f")
            .arg(&self.pom_path)
            .output()
            .context("Failed to execute mvn dependency:tree")?;

        if !output.status.success() {
            warn!("mvn dependency:tree failed, falling back to pom.xml parsing");
            debug!("Maven stderr: {}", String::from_utf8_lossy(&output.stderr));
            return self.parse_pom_dependencies();
        }

        let tree_output = String::from_utf8_lossy(&output.stdout);
        let dependencies = self.parse_dependency_tree(&tree_output);

        info!("Resolved {} dependencies (including transitive)", dependencies.len());

        // If mvn returned no dependencies, fall back to parsing pom.xml
        if dependencies.is_empty() {
            info!("No dependencies found in mvn output, falling back to pom.xml parsing");
            return self.parse_pom_dependencies();
        }

        Ok(dependencies)
    }

    /// Parse dependencies directly from pom.xml
    fn parse_pom_dependencies(&self) -> Result<Vec<MavenDependency>> {
        info!("Parsing pom.xml directly: {}", self.pom_path.display());
        let pom = MavenPom::parse(&self.pom_path)?;
        info!("Found {} dependencies in pom.xml", pom.dependencies.len());
        for dep in &pom.dependencies {
            debug!("  - {}:{}", dep.group_id, dep.artifact_id);
        }
        Ok(pom.dependencies)
    }

    /// Parse the output of mvn dependency:tree
    fn parse_dependency_tree(&self, output: &str) -> Vec<MavenDependency> {
        let mut dependencies = Vec::new();

        for line in output.lines() {
            // Look for dependency lines like:
            // [INFO] +- groupId:artifactId:type:version:scope
            // [INFO] \- groupId:artifactId:type:version:scope
            if !line.contains('+') && !line.contains('\\') {
                continue;
            }

            // Extract the dependency part after the tree symbols
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }

            // Find the dependency coordinate (format: groupId:artifactId:type:version:scope)
            let coord_idx = parts.iter().position(|&p| p.contains(':'));
            if coord_idx.is_none() {
                continue;
            }

            let coord = parts[coord_idx.unwrap()];
            if let Some(dep) = self.parse_dependency_coordinate(coord) {
                dependencies.push(dep);
            }
        }

        dependencies
    }

    /// Parse a Maven coordinate string (groupId:artifactId:type:version:scope)
    fn parse_dependency_coordinate(&self, coord: &str) -> Option<MavenDependency> {
        let parts: Vec<&str> = coord.split(':').collect();

        if parts.len() < 4 {
            return None;
        }

        let group_id = parts[0].to_string();
        let artifact_id = parts[1].to_string();
        let type_ = if parts.len() >= 3 {
            Some(parts[2].to_string())
        } else {
            None
        };
        let version = if parts.len() >= 4 {
            Some(parts[3].to_string())
        } else {
            None
        };
        let scope = if parts.len() >= 5 {
            Some(parts[4].to_string())
        } else {
            None
        };

        Some(MavenDependency {
            group_id,
            artifact_id,
            version,
            scope,
            classifier: None,
            type_,
            optional: false,
        })
    }
}

/// Find all pom.xml files in a directory tree
pub fn find_pom_files(path: &Path) -> Result<Vec<PathBuf>> {
    let mut pom_files = Vec::new();

    if path.is_file() && path.file_name().map_or(false, |name| name == "pom.xml") {
        pom_files.push(path.to_path_buf());
    } else if path.is_dir() {
        for entry in std::fs::read_dir(path)
            .with_context(|| format!("Failed to read directory: {}", path.display()))?
        {
            let entry = entry?;
            let entry_path = entry.path();

            // Skip target and .m2 directories
            if entry_path.is_dir() {
                if let Some(name) = entry_path.file_name() {
                    if name == "target" || name == ".m2" {
                        continue;
                    }
                }
            }

            pom_files.extend(find_pom_files(&entry_path)?);
        }
    }

    Ok(pom_files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_pom() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <artifactId>my-app</artifactId>
    <version>1.0.0</version>
    <packaging>jar</packaging>

    <dependencies>
        <dependency>
            <groupId>junit</groupId>
            <artifactId>junit</artifactId>
            <version>4.13.2</version>
            <scope>test</scope>
        </dependency>
    </dependencies>
</project>"#;

        let pom = MavenPom::parse_from_string(xml, PathBuf::from("pom.xml")).unwrap();

        assert_eq!(pom.group_id, Some("com.example".to_string()));
        assert_eq!(pom.artifact_id, Some("my-app".to_string()));
        assert_eq!(pom.version, Some("1.0.0".to_string()));
        assert_eq!(pom.packaging, Some("jar".to_string()));
        assert_eq!(pom.dependencies.len(), 1);

        let dep = &pom.dependencies[0];
        assert_eq!(dep.group_id, "junit");
        assert_eq!(dep.artifact_id, "junit");
        assert_eq!(dep.version, Some("4.13.2".to_string()));
        assert_eq!(dep.scope, Some("test".to_string()));
    }

    #[test]
    fn test_parse_pom_with_parent() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <modelVersion>4.0.0</modelVersion>

    <parent>
        <groupId>org.springframework.boot</groupId>
        <artifactId>spring-boot-starter-parent</artifactId>
        <version>2.7.0</version>
    </parent>

    <groupId>com.example</groupId>
    <artifactId>my-app</artifactId>
    <version>1.0.0</version>
</project>"#;

        let pom = MavenPom::parse_from_string(xml, PathBuf::from("pom.xml")).unwrap();

        assert!(pom.parent.is_some());
        let parent = pom.parent.unwrap();
        assert_eq!(parent.group_id, "org.springframework.boot");
        assert_eq!(parent.artifact_id, "spring-boot-starter-parent");
        assert_eq!(parent.version, "2.7.0");
    }

    #[test]
    fn test_dependency_to_identifier() {
        let dep = MavenDependency {
            group_id: "junit".to_string(),
            artifact_id: "junit".to_string(),
            version: Some("4.13.2".to_string()),
            scope: Some("test".to_string()),
            classifier: None,
            type_: None,
            optional: false,
        };

        assert_eq!(dep.to_identifier(), "junit:junit:4.13.2");
        assert_eq!(dep.name(), "junit");
    }
}
