use aipack::detection::tools::ToolRegistry;

fn main() {
    println!("Tool Registry - JSON Schema Inspection\n");
    println!("{}", "=".repeat(80));
    println!();

    let tools = ToolRegistry::create_all_tools();

    for (i, tool) in tools.iter().enumerate() {
        println!("{}. {} Tool", i + 1, tool.name.to_uppercase());
        println!("{}", "-".repeat(80));

        if let Some(desc) = &tool.description {
            println!("Description: {}", desc);
        }

        if let Some(schema) = &tool.schema {
            println!("\nJSON Schema:");
            println!("{}", serde_json::to_string_pretty(schema).unwrap());
        }

        println!("\n{}\n", "=".repeat(80));
    }
}
