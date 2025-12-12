use aipack::tools::ToolRegistry;
use std::path::PathBuf;

fn main() {
    println!("Creating all tools from ToolRegistry...\n");

    let repo_path = PathBuf::from(".");
    let registry = ToolRegistry::new(repo_path).expect("Failed to create tool registry");

    println!("Total tools registered: {}\n", registry.len());

    let tool_definitions = registry.as_tool_definitions();
    for def in tool_definitions {
        println!("Tool: {}", def.name);
        println!("  Description: {}", def.description);
        if let Some(schema_obj) = def.parameters.as_object() {
            if let Some(required) = schema_obj.get("required").and_then(|r| r.as_array()) {
                println!("  Required parameters: {}", required.len());
            }
        }
        println!();
    }
}
