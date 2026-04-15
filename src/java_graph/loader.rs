// Graph loading from Java source files using tree-sitter-stack-graphs

use anyhow::{Context, Result};
use stack_graphs::graph::StackGraph;
use std::path::Path;
use tree_sitter_stack_graphs::{NoCancellation, Variables};

use super::language_config;

/// Load TSG rules for Java
pub fn load_tsg_rules() -> Result<tree_sitter_stack_graphs::StackGraphLanguage> {
    let tsg_source = include_str!("stack-graphs.tsg");
    let language = language_config::language();

    tree_sitter_stack_graphs::StackGraphLanguage::from_str(language, tsg_source)
        .context("Failed to load Java stack-graphs TSG rules")
}

/// Build a stack graph for a single Java source file
pub fn build_graph_for_file(
    file_path: &Path,
    graph: &mut StackGraph,
    tsg: &tree_sitter_stack_graphs::StackGraphLanguage,
) -> Result<()> {
    use stack_graphs::graph::NodeID;

    let (source, _tree) = language_config::parse_file(file_path)?;

    let file_handle = graph.get_or_create_file(file_path.to_string_lossy().as_ref());

    // Create a builder for the stack graph
    let mut builder = tsg.builder_into_stack_graph(graph, file_handle, &source);

    // Create special nodes for organizing the stack graph structure
    // These match the global variables referenced in the TSG file
    let source_type_node_id = NodeID::new_in_file(file_handle, 0);
    let source_type_node = builder.inject_node(source_type_node_id);

    let jump_to_scope_node_id = NodeID::new_in_file(file_handle, 1);
    let jump_to_scope_node = builder.inject_node(jump_to_scope_node_id);

    let root_node_id = NodeID::new_in_file(file_handle, 2);
    let root_node = builder.inject_node(root_node_id);

    // Set global variables
    let relative_path = file_path.to_str().context("Invalid file path")?;
    let mut globals = Variables::new();
    globals.add("FILE_PATH".into(), relative_path.into()).ok();
    globals.add("PROJECT_NAME".into(), "".into()).ok();
    globals.add("ROOT_PATH".into(), "".into()).ok();
    globals.add("SOURCE_TYPE_NODE".into(), source_type_node.into()).ok();
    globals.add("JUMP_TO_SCOPE_NODE".into(), jump_to_scope_node.into()).ok();
    globals.add("ROOT_NODE".into(), root_node.into()).ok();

    // Build the stack graph
    builder.build(&globals, &NoCancellation)
        .map_err(|e| anyhow::anyhow!("Stack graph build error: {}", e))?;

    Ok(())
}

/// Build a stack graph for multiple Java source files
pub fn build_graph_for_files(
    files: &[&Path],
) -> Result<StackGraph> {
    let tsg = load_tsg_rules()?;
    let mut graph = StackGraph::new();

    for file in files {
        build_graph_for_file(file, &mut graph, &tsg)
            .with_context(|| format!("Failed to build graph for {}", file.display()))?;
    }

    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_load_tsg_rules() {
        let result = load_tsg_rules();
        assert!(result.is_ok(), "Failed to load TSG rules: {:?}", result.err());
    }

    #[test]
    fn test_build_graph_simple() {
        let fixture_path = PathBuf::from("tests/fixtures/Simple.java");
        if !fixture_path.exists() {
            eprintln!("Skipping test - fixture not found: {}", fixture_path.display());
            return;
        }

        let result = build_graph_for_files(&[&fixture_path]);
        assert!(result.is_ok(), "Failed to build graph: {:?}", result.err());

        let graph = result.unwrap();
        // Verify the graph was created
        assert!(graph.iter_files().count() > 0);
    }
}
