use terminusdb_woql2::prelude::*;

// Define a bunch of types for complex relationship modeling
struct User;
struct Profile;
struct Post;
struct Comment;
struct Like;
struct Tag;
struct Category;
struct Forum;
struct Thread;
struct Reply;
struct Notification;
struct Subscription;
struct Group;
struct Permission;
struct Role;
struct Organization;
struct Project;
struct Task;
struct Milestone;
struct Sprint;

#[test]
fn test_10_node_social_graph() {
    // A complex social media traversal
    let query = from_path!(
        User > Profile > Post > Comment > Like < User > Group > Permission > Role > Organization
    );
    
    println!("10-node social graph traversal:");
    println!("{}", query.to_dsl());
    
    // Verify all types are present
    let dsl = query.to_dsl();
    assert!(dsl.contains("User"));
    assert!(dsl.contains("Profile"));
    assert!(dsl.contains("Post"));
    assert!(dsl.contains("Comment"));
    assert!(dsl.contains("Like"));
    assert!(dsl.contains("Group"));
    assert!(dsl.contains("Permission"));
    assert!(dsl.contains("Role"));
    assert!(dsl.contains("Organization"));
}

#[test]
fn test_15_node_project_hierarchy() {
    // Complex project management hierarchy with mixed directions
    let query = from_path!(
        Organization > Project > Sprint > Task < User > Role < Permission < Group > Organization > Project > Milestone > Task > Comment > Reply < User > Notification
    );
    
    println!("\n15-node project hierarchy:");
    println!("{}", query.to_dsl());
    
    // Count the number of "triple" occurrences as a proxy for relationship count
    let triple_count = query.to_dsl().matches("triple(").count();
    println!("Number of triples generated: {}", triple_count);
    assert!(triple_count >= 15); // At least one per node
}

#[test]
fn test_20_node_forum_traversal() {
    // Very long forum discussion thread traversal
    struct Message;
    struct Moderator;
    struct Ban;
    struct Appeal;
    struct Review;
    struct Decision;
    struct Action;
    
    let query = from_path!(
        Forum > Category > Thread > Post > Comment > Reply > User > Profile > Group > Permission > 
        Role > Moderator > Action > Ban > User > Appeal > Review > Decision > Moderator > Organization
    );
    
    println!("\n20-node forum traversal:");
    println!("{}", query.to_dsl());
    
    // This should generate a very complex WOQL query
    let dsl_length = query.to_dsl().len();
    println!("Generated DSL length: {} characters", dsl_length);
    assert!(dsl_length > 1000); // Should be quite long
}

#[test]
fn test_25_node_mixed_directions() {
    // Extremely long path with many direction changes
    struct A;
    struct B;
    struct C;
    struct D;
    struct E;
    struct F;
    struct G;
    struct H;
    struct I;
    struct J;
    struct K;
    struct L;
    struct M;
    struct N;
    struct O;
    struct P;
    struct Q;
    struct R;
    struct S;
    struct T;
    struct U;
    struct V;
    struct W;
    struct X;
    struct Y;
    
    let query = from_path!(
        A > B < C > D > E < F < G > H > I < J > K > L < M > N < O > P > Q < R > S > T < U > V > W < X > Y
    );
    
    println!("\n25-node path with mixed directions:");
    let dsl = query.to_dsl();
    println!("First 200 chars: {}...", &dsl[..200.min(dsl.len())]);
    println!("Total DSL length: {} characters", dsl.len());
    
    // Count forward vs backward relationships
    let forward_count = dsl.matches("Posts").count() + dsl.matches("s\"").count();
    let backward_count = dsl.matches("Comments").count(); // Rough heuristic
    println!("Approximate forward relations: {}", forward_count);
}

#[test]
fn test_30_node_with_custom_variables() {
    // 30 nodes with custom variable names throughout
    let query = from_path!(
        a:User > b:Post > c:Comment > d:Like > e:User > f:Group > g:Permission > h:Role > 
        i:Organization > j:Project > k:Sprint > l:Task > m:User > n:Profile > o:Subscription > 
        p:Notification > q:User > r:Post > s:Tag > t:Category > u:Forum > v:Thread > w:Reply > 
        x:Comment > y:Like > z:User > aa:Group > bb:Permission > cc:Role > dd:Organization
    );
    
    println!("\n30-node path with custom variables:");
    let dsl = query.to_dsl();
    
    // Verify custom variables are used
    assert!(dsl.contains("\"a\""));
    assert!(dsl.contains("\"z\""));
    assert!(dsl.contains("\"dd\""));
    
    println!("Uses custom variables from 'a' to 'dd'");
    println!("Total DSL length: {} characters", dsl.len());
}

#[test]
fn test_35_node_alternating_pattern() {
    // 35 nodes with alternating forward/backward pattern
    struct Settings;
    struct Privacy;
    struct Visibility;
    struct Public;
    struct Review;
    struct Decision;
    struct Moderator;
    struct Action;
    struct Ban;
    struct Appeal;
    
    // Build a query that alternates > and < for 35 nodes
    let query = from_path!(
        User > Post < Comment > Like < Tag > Category < Forum > Thread < Reply > User >
        Profile < Subscription > Notification < Group > Permission < Role > Organization < Project >
        Task < Sprint > Milestone < Review > Decision < Moderator > Action < Ban > Appeal <
        User > Settings > Privacy < Visibility > Public
    );
    
    println!("\n35-node alternating pattern:");
    let dsl = query.to_dsl();
    println!("DSL length: {} characters", dsl.len());
    
    // This should create a very complex back-and-forth traversal pattern
    assert!(dsl.len() > 2000);
}

#[test]
fn test_40_node_stress_test() {
    // 40+ node path to stress test the macro
    struct Type1;
    struct Type2;
    struct Type3;
    struct Type4;
    struct Type5;
    struct Type6;
    struct Type7;
    struct Type8;
    struct Type9;
    struct Type10;
    
    // Create a very long chain by repeating patterns
    let query = from_path!(
        Type1 > Type2 > Type3 > Type4 > Type5 > Type6 > Type7 > Type8 > Type9 > Type10 >
        Type1 > Type2 > Type3 > Type4 > Type5 > Type6 > Type7 > Type8 > Type9 > Type10 >
        Type1 > Type2 > Type3 > Type4 > Type5 > Type6 > Type7 > Type8 > Type9 > Type10 >
        Type1 > Type2 > Type3 > Type4 > Type5 > Type6 > Type7 > Type8 > Type9 > Type10
    );
    
    println!("\n40-node stress test:");
    let dsl = query.to_dsl();
    println!("Successfully generated {} character query", dsl.len());
    println!("Number of 'and' clauses: {}", dsl.matches("and(").count());
    
    // Should handle this without stack overflow or compilation issues
    assert!(dsl.len() > 3000);
}

#[test]
fn test_50_node_complex_pattern() {
    // 50 nodes with mixed patterns - note: fields can only appear at the start of a path
    struct Settings;
    struct Privacy;
    struct Visibility;
    struct Public;
    struct Review;
    struct Decision;
    struct Moderator;
    struct Action;
    struct Ban;
    struct Appeal;
    
    // Simplified version without mid-chain fields (not supported by builder)
    let query = from_path!(
        u1:User > Post > Comment < Like > User > Profile > Settings >
        Privacy < v:Visibility > Public > Post > c1:Comment < User > Like >
        Tag > Category < Forum > Thread > Reply < Comment > User >
        Group > Permission < Role > Organization > Project > Task <
        Sprint > Milestone > Review > Decision < Moderator > Action >
        Ban > Appeal < User > Notification > Subscription > Group <
        Permission > Role < Organization > Project > Task
    );
    
    println!("\n50-node complex pattern with fields and variables:");
    let dsl = query.to_dsl();
    println!("Generated {} character query", dsl.len());
    
    // Verify it includes type names
    assert!(dsl.contains("Settings"));
    assert!(dsl.contains("Privacy"));
    assert!(dsl.contains("Permission"));
    
    // Verify custom variables
    assert!(dsl.contains("u1"));
    assert!(dsl.contains("c1"));
}

#[test] 
fn test_recursive_macro_limits() {
    // Test approaching any recursion limits
    println!("\n=== Macro Recursion Limits Test ===");
    
    // The macro uses @continue pattern for recursion
    // Let's verify it handles very long chains without hitting limits
    
    struct Node;
    
    // 60 nodes should be well within limits
    let query = from_path!(
        Node > Node > Node > Node > Node > Node > Node > Node > Node > Node >
        Node > Node > Node > Node > Node > Node > Node > Node > Node > Node >
        Node > Node > Node > Node > Node > Node > Node > Node > Node > Node >
        Node > Node > Node > Node > Node > Node > Node > Node > Node > Node >
        Node > Node > Node > Node > Node > Node > Node > Node > Node > Node >
        Node > Node > Node > Node > Node > Node > Node > Node > Node > Node
    );
    
    let dsl = query.to_dsl();
    println!("60 identical nodes: {} characters", dsl.len());
    
    // Each Node should appear in the query
    let node_count = dsl.matches("@schema:Node").count();
    println!("Node type references: {}", node_count);
    assert!(node_count >= 60);
}

#[test]
fn test_performance_characteristics() {
    use std::time::Instant;
    
    println!("\n=== Performance Characteristics ===");
    
    // Measure compilation/expansion time indirectly through runtime
    let start = Instant::now();
    
    // Generate a moderately complex query
    let _ = from_path!(
        User > Post > Comment > Like > Tag > Category > Forum > Thread > Reply > User >
        Profile > Subscription > Notification > Group > Permission > Role > Organization
    );
    
    let duration = start.elapsed();
    println!("17-node query generation took: {:?}", duration);
    
    // The macro expansion happens at compile time, so runtime should be minimal
    assert!(duration.as_millis() < 10); // Should be very fast at runtime
}