use aipack::detection::tools::ToolRegistry;

fn main() {
    println!("Creating all tools from ToolRegistry...\n");

    let tools = ToolRegistry::create_all_tools();
    println!("Total tools created: {}\n", tools.len());

    for tool in tools {
        println!("Tool: {}", tool.name);
        if let Some(desc) = &tool.description {
            println!("  Description: {}", desc);
        }
        if let Some(schema) = &tool.schema {
            println!("  Schema type: {}", schema["type"]);
            if let Some(required) = schema["required"].as_array() {
                println!("  Required parameters: {}", required.len());
            }
        }
        println!();
    }
}
