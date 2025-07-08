//! Example demonstrating how to retrieve instances along with their commit IDs

use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{TerminusDBModel, FromTDBInstance};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Article {
    id: EntityIDFor<Self>,
    title: String,
    content: String,
    author: String,
    published: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Connect to local TerminusDB instance
    let client = TerminusDBHttpClient::local_node();
    let spec = BranchSpec::from("admin/blog/main");
    
    // Insert article schema
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Article>(args.clone()).await?;
    
    // Create an article
    let article = Article {
        id: EntityIDFor::new("rust-terminusdb-headers")?,
        title: "Working with TerminusDB Headers in Rust".to_string(),
        content: "Learn how to retrieve commit IDs from GET operations...".to_string(),
        author: "Tech Writer".to_string(),
        published: true,
    };
    
    // Insert the article
    let insert_result = client.save_instance(&article, args).await?;
    println!("Article inserted successfully");
    
    // Retrieve the article with commit ID
    let mut deserializer = terminusdb_client::deserialize::DefaultTDBDeserializer;
    let result = client.get_instance_with_headers::<Article>(
        "rust-terminusdb-headers",
        &spec,
        &mut deserializer
    ).await?;
    
    // Access the article via Deref
    let retrieved_article = &*result;
    
    println!("\nRetrieved article: {}", retrieved_article.title);
    println!("Author: {}", retrieved_article.author);
    
    if let Some(commit) = result.extract_commit_id() {
        println!("\nCommit information:");
        println!("  Current commit: {}", commit);
        
        // You can use this commit ID for:
        // 1. Auditing - know exactly which version you're looking at
        // 2. Caching - use commit ID as cache key
        // 3. Time travel - construct a commit-specific BranchSpec
        
        // Example: Create a commit-specific spec for time travel
        let commit_spec = BranchSpec::from(&format!("admin/blog/local/commit/{}", commit));
        println!("\nTime travel spec: {:?}", commit_spec);
        
        // This commit ID can also be used to verify data consistency
        // or to track when data was last modified
    } else {
        println!("\nNo commit ID found in response headers");
    }
    
    // Demonstrate bulk retrieval with commit ID
    println!("\n--- Bulk Retrieval Example ---");
    
    // Create more articles
    let articles = vec![
        Article {
            id: EntityIDFor::new("intro-to-graphs")?,
            title: "Introduction to Graph Databases".to_string(),
            content: "Graph databases are powerful...".to_string(),
            author: "DB Expert".to_string(),
            published: true,
        },
        Article {
            id: EntityIDFor::new("api-design-tips")?,
            title: "API Design Best Practices".to_string(),
            content: "When designing APIs...".to_string(),
            author: "API Guru".to_string(),
            published: false,
        },
    ];
    
    // Insert multiple articles
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_instances(articles, args).await?;
    
    // Retrieve multiple articles with commit ID
    let article_ids = vec![
        "rust-terminusdb-headers".to_string(),
        "intro-to-graphs".to_string(),
    ];
    
    let opts = GetOpts::default();
    let result = client.get_instances_with_headers::<Article>(
        article_ids,
        &spec,
        opts,
        &mut deserializer
    ).await?;
    
    // Access the articles via Deref
    let articles = &*result;
    
    println!("\nRetrieved {} articles", articles.len());
    for article in articles {
        println!("  - {}", article.title);
    }
    
    if let Some(commit) = result.extract_commit_id() {
        println!("\nAll articles retrieved from commit: {}", commit);
    }
    
    // Demonstrate the simplified get_latest_version method
    println!("\n--- Get Latest Version Example ---");
    
    let latest_commit = client.get_latest_version::<Article>("rust-terminusdb-headers", &spec).await?;
    println!("Latest version of article is at commit: {}", latest_commit);
    
    Ok(())
}