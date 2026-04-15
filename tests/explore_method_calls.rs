use java_analyzer_provider::java_graph::{ast_explorer, language_config};

#[test]
#[ignore]
fn explore_method_invocation_structure() {
    let source = r#"
        public class Test {
            public void example() {
                userService.getUserName();
                System.out.println("test");
                items.add("test").toString();
            }
        }
    "#;

    let tree = language_config::parse_source(source).unwrap();

    println!("\n=== Method Invocation AST ===");
    let method_invocations = ast_explorer::find_nodes_by_kind(&tree, "method_invocation");

    for (i, node) in method_invocations.iter().enumerate() {
        println!("\n--- Method Invocation {} ---", i + 1);
        println!("Text: {}", ast_explorer::node_text(*node, source));

        for child in node.children(&mut node.walk()) {
            println!("  Child: {} = {}", child.kind(), ast_explorer::node_text(child, source));
        }
    }
}

#[test]
#[ignore]
fn explore_object_creation_structure() {
    let source = r#"
        public class Test {
            public void example() {
                User user = new User("test", 30);
                List<String> list = new ArrayList<>();
            }
        }
    "#;

    let tree = language_config::parse_source(source).unwrap();

    println!("\n=== Object Creation AST ===");
    let object_creations = ast_explorer::find_nodes_by_kind(&tree, "object_creation_expression");

    for (i, node) in object_creations.iter().enumerate() {
        println!("\n--- Object Creation {} ---", i + 1);
        println!("Text: {}", ast_explorer::node_text(*node, source));

        for child in node.children(&mut node.walk()) {
            println!("  Child: {} = {}", child.kind(), ast_explorer::node_text(child, source));
        }
    }
}
