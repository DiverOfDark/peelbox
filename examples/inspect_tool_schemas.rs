use aipack::tools::ToolRegistry;
use std::path::PathBuf;

fn main() {
    println!("Tool Registry - JSON Schema Inspection\n");
    println!("{}", "=".repeat(80));
    println!();

    let repo_path = PathBuf::from(".");
    let registry = ToolRegistry::new(repo_path)
        .expect("Failed to create tool registry");

    let tool_definitions = registry.as_tool_definitions();

    for (i, def) in tool_definitions.iter().enumerate() {
        println!("{}. {} Tool", i + 1, def.name.to_uppercase());
        println!("{}", "-".repeat(80));

        println!("Description: {}", def.description);

        println!("\nJSON Schema:");
        println!("{}", serde_json::to_string_pretty(&def.parameters).unwrap());

        println!("\n{}\n", "=".repeat(80));
    }
}
