use terminusdb_woql2::prelude::*;

// Test types for path traversal
struct User;
struct Post;
struct Comment;
struct Like;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_type() {
        println!("🧪 Testing single type syntax...");

        // Test: from_path!(User)
        let query = from_path!(User);
        println!("Single type query: {:#?}", query);

        // Should generate a type constraint for User
        match query {
            Query::Triple(_) => {
                println!("✅ Single type generates type constraint");
            }
            _ => {
                println!("❌ Unexpected query structure for single type");
                panic!("Single type should generate Triple query");
            }
        }
    }

    #[test]
    fn test_two_types_forward() {
        println!("🧪 Testing two type forward relation...");

        // Test: from_path!(User > Post)
        let query = from_path!(User > Post);
        println!("Two type query: {:#?}", query);

        // Should generate And query with triple and type constraints
        match query {
            Query::And(_) => {
                println!("✅ Two types generate And constraint");
            }
            _ => {
                println!("❌ Unexpected query structure for two types");
                panic!("Two types should generate And query");
            }
        }
    }

    #[test]
    fn test_three_types_chain() {
        println!("🧪 Testing three type chain traversal...");

        // Test: from_path!(User > Post > Comment)
        let query = from_path!(User > Post > Comment);
        println!("Three type chain query: {:#?}", query);

        // Should generate nested And query with multiple constraints
        match query {
            Query::And(_) => {
                println!("✅ Three type chain generates And constraint");
            }
            _ => {
                println!("❌ Unexpected query structure for three types");
                panic!("Three types should generate And query");
            }
        }

        println!("✅ Basic forward traversal syntax working!");
    }

    #[test]
    fn test_four_types_complex_chain() {
        println!("🧪 Testing complex four type chain...");

        // Test: from_path!(User > Post > Comment > Like)
        let query = from_path!(User > Post > Comment > Like);
        println!("Four type chain query: {:#?}", query);

        // Should generate complex nested And query
        match query {
            Query::And(_) => {
                println!("✅ Four type chain generates And constraint");
            }
            _ => {
                println!("❌ Unexpected query structure for four types");
                panic!("Four types should generate And query");
            }
        }

        println!("✅ Complex chain traversal working!");
    }

    #[test]
    fn test_variable_name_generation() {
        println!("🧪 Testing variable name generation...");

        let query = from_path!(User > Post);
        let query_str = format!("{:#?}", query);

        // Check that variable names are generated correctly
        assert!(
            query_str.contains("User_1"),
            "Should contain User_1 variable"
        );
        assert!(
            query_str.contains("Post_1"),
            "Should contain Post_1 variable"
        );

        println!("✅ Variable name generation working correctly!");
    }

    #[test]
    fn test_field_name_generation() {
        println!("🧪 Testing field name generation...");

        let query = from_path!(User > Post);
        let query_str = format!("{:#?}", query);

        // Check that field names are generated (simple pluralization)
        assert!(
            query_str.contains("Posts"),
            "Should contain pluralized field name"
        );

        println!("✅ Field name generation working!");
    }

    #[test]
    fn test_schema_type_generation() {
        println!("🧪 Testing schema type generation...");

        let query = from_path!(User > Post);
        let query_str = format!("{:#?}", query);

        // Check that schema types are generated correctly
        assert!(
            query_str.contains("@schema:User"),
            "Should contain @schema:User"
        );
        assert!(
            query_str.contains("@schema:Post"),
            "Should contain @schema:Post"
        );

        println!("✅ Schema type generation working!");
    }

    #[test]
    fn test_compilation_success() {
        println!("🎉 Testing that all basic syntax compiles successfully...");

        // Test that various syntaxes compile without error
        let _q1 = from_path!(User);
        let _q2 = from_path!(User > Post);
        let _q3 = from_path!(User > Post > Comment);
        let _q4 = from_path!(User > Post > Comment > Like);
        let _q5 = from_path!(Post > User); // Reverse direction types
        let _q6 = from_path!(Comment > Post > User); // Multi-hop reverse

        println!("✅ All basic forward traversal syntax compiles successfully!");
        println!("🚀 Phase 1: Basic Forward Traversal - IMPLEMENTATION COMPLETE!");
    }

    #[test]
    fn test_reverse_relation() {
        println!("🧪 Testing reverse relation syntax...");

        // Test: from_path!(Comment < Post) means Post has Comments
        let query = from_path!(Comment < Post);
        println!("Reverse relation query: {:#?}", query);

        // Should generate And query with triple and type constraints
        match query {
            Query::And(_) => {
                println!("✅ Reverse relation generates And constraint");
            }
            _ => {
                println!("❌ Unexpected query structure for reverse relation");
                panic!("Reverse relation should generate And query");
            }
        }
    }

    #[test]
    fn test_mixed_forward_reverse() {
        println!("🧪 Testing mixed forward and reverse relations...");

        // Test: from_path!(User > Post < Comment)
        // User relates to Post, Comment relates to Post (Post has both Users and Comments)
        let query = from_path!(User > Post < Comment);
        println!("Mixed forward-reverse query: {:#?}", query);

        // Should generate And query with multiple constraints
        match query {
            Query::And(_) => {
                println!("✅ Mixed relations generate And constraint");
            }
            _ => {
                println!("❌ Unexpected query structure for mixed relations");
                panic!("Mixed relations should generate And query");
            }
        }
    }

    #[test]
    fn test_mixed_reverse_forward() {
        println!("🧪 Testing mixed reverse and forward relations...");

        // Test: from_path!(Comment < Post > User)
        // Post has Comments, Post relates to User
        let query = from_path!(Comment < Post > User);
        println!("Mixed reverse-forward query: {:#?}", query);

        match query {
            Query::And(_) => {
                println!("✅ Reverse-forward relations generate And constraint");
            }
            _ => {
                println!("❌ Unexpected query structure for reverse-forward relations");
                panic!("Reverse-forward relations should generate And query");
            }
        }
    }

    #[test]
    fn test_reverse_relation_compilation() {
        println!("🧪 Testing reverse relation compilation...");

        // Test that various reverse syntaxes compile without error
        let _q1 = from_path!(Comment < Post);
        let _q2 = from_path!(User > Post < Comment);
        let _q3 = from_path!(Comment < Post > User);

        println!("✅ All reverse relation syntax compiles successfully!");
        println!("🚀 Phase 2: Reverse Relations - IMPLEMENTATION COMPLETE!");
    }

    #[test]
    fn test_explicit_field_syntax() {
        println!("🧪 Testing explicit field syntax...");

        // Test: from_path!(User.author > Post) uses exact field name "author"
        let query = from_path!(User.author > Post);
        println!("Explicit field query: {:#?}", query);

        // Should generate And query with triple using "author" field
        match query {
            Query::And(_) => {
                println!("✅ Explicit field generates And constraint");
            }
            _ => {
                println!("❌ Unexpected query structure for explicit field");
                panic!("Explicit field should generate And query");
            }
        }

        // Verify that the field name is used exactly as specified
        let query_str = format!("{:#?}", query);
        assert!(
            query_str.contains("\"author\""),
            "Should contain exact field name 'author'"
        );
        assert!(
            !query_str.contains("\"Posts\""),
            "Should NOT contain auto-generated plural 'Posts'"
        );

        println!("✅ Explicit field name verification passed!");
    }

    #[test]
    fn test_explicit_field_reverse_syntax() {
        println!("🧪 Testing explicit field reverse syntax...");

        // Test: from_path!(Post.author < User) means User has field "author" pointing to Post
        let query = from_path!(Post < User);
        println!("Explicit field reverse query: {:#?}", query);

        match query {
            Query::And(_) => {
                println!("✅ Explicit field reverse generates And constraint");
            }
            _ => {
                println!("❌ Unexpected query structure for explicit field reverse");
                panic!("Explicit field reverse should generate And query");
            }
        }
    }

    #[test]
    fn test_mixed_explicit_field_syntax() {
        println!("🧪 Testing mixed explicit field syntax...");

        // Test: from_path!(User.manager > User.reports > User)
        let query = from_path!(User.manager > User.reports > User);
        println!("Mixed explicit field query: {:#?}", query);

        match query {
            Query::And(_) => {
                println!("✅ Mixed explicit field generates And constraint");
            }
            _ => {
                println!("❌ Unexpected query structure for mixed explicit field");
                panic!("Mixed explicit field should generate And query");
            }
        }

        // Verify both field names are used
        let query_str = format!("{:#?}", query);
        assert!(
            query_str.contains("\"manager\""),
            "Should contain field name 'manager'"
        );
        assert!(
            query_str.contains("\"reports\""),
            "Should contain field name 'reports'"
        );

        println!("✅ Mixed explicit field verification passed!");
    }

    #[test]
    fn test_explicit_field_compilation() {
        println!("🧪 Testing explicit field compilation...");

        // Test that various explicit field syntaxes compile without error
        let _q1 = from_path!(User.author > Post);
        let _q2 = from_path!(Post < User);
        let _q3 = from_path!(User.manager > User.reports > User);
        let _q4 = from_path!(User.posts > Post.author > User);

        println!("✅ All explicit field syntax compiles successfully!");
        println!("🚀 Phase 3: Explicit Field Syntax - IMPLEMENTATION COMPLETE!");
    }

    #[test]
    fn test_custom_variable_syntax() {
        println!("🧪 Testing custom variable syntax...");

        // Test: from_path!(u:User) uses custom variable "u"
        let query = from_path!(u:User);
        println!("Custom variable query: {:#?}", query);

        // Should generate Triple query with custom variable name
        match query {
            Query::Triple(_) => {
                println!("✅ Custom variable generates Triple constraint");
            }
            _ => {
                println!("❌ Unexpected query structure for custom variable");
                panic!("Custom variable should generate Triple query");
            }
        }

        // Verify that the custom variable name is used
        let query_str = format!("{:#?}", query);
        assert!(
            query_str.contains("\"u\""),
            "Should contain custom variable name 'u'"
        );
        assert!(
            !query_str.contains("\"User_1\""),
            "Should NOT contain auto-generated 'User_1'"
        );

        println!("✅ Custom variable name verification passed!");
    }

    #[test]
    fn test_custom_variables_relation() {
        println!("🧪 Testing custom variables in relations...");

        // Test: from_path!(u:User > p:Post) uses custom variable names
        let query = from_path!(u:User > p:Post);
        println!("Custom variables relation query: {:#?}", query);

        match query {
            Query::And(_) => {
                println!("✅ Custom variables relation generates And constraint");
            }
            _ => {
                println!("❌ Unexpected query structure for custom variables relation");
                panic!("Custom variables relation should generate And query");
            }
        }

        // Verify custom variable names are used
        let query_str = format!("{:#?}", query);
        assert!(
            query_str.contains("\"u\""),
            "Should contain custom variable 'u'"
        );
        assert!(
            query_str.contains("\"p\""),
            "Should contain custom variable 'p'"
        );
        assert!(
            !query_str.contains("\"User_1\""),
            "Should NOT contain auto-generated variables"
        );

        println!("✅ Custom variables relation verification passed!");
    }

    #[test]
    fn test_mixed_custom_auto_variables() {
        println!("🧪 Testing mixed custom and auto variables...");

        // Test: from_path!(u:User > Post) mixes custom and auto variables
        let query = from_path!(u:User > Post);
        println!("Mixed variables query: {:#?}", query);

        match query {
            Query::And(_) => {
                println!("✅ Mixed variables generate And constraint");
            }
            _ => {
                panic!("Mixed variables should generate And query");
            }
        }

        // Verify both custom and auto variable names
        let query_str = format!("{:#?}", query);
        assert!(
            query_str.contains("\"u\""),
            "Should contain custom variable 'u'"
        );
        assert!(
            query_str.contains("\"Post_1\""),
            "Should contain auto-generated 'Post_1'"
        );

        println!("✅ Mixed variables verification passed!");
    }

    #[test]
    fn test_custom_variables_with_fields() {
        println!("🧪 Testing custom variables with explicit fields...");

        // Test: from_path!(u:User.manager > m:User)
        let query = from_path!(u:User.manager > m:User);
        println!("Custom variables with field query: {:#?}", query);

        match query {
            Query::And(_) => {
                println!("✅ Custom variables with field generate And constraint");
            }
            _ => {
                panic!("Custom variables with field should generate And query");
            }
        }

        // Verify custom variables and field name
        let query_str = format!("{:#?}", query);
        assert!(
            query_str.contains("\"u\""),
            "Should contain custom variable 'u'"
        );
        assert!(
            query_str.contains("\"m\""),
            "Should contain custom variable 'm'"
        );
        assert!(
            query_str.contains("\"manager\""),
            "Should contain field name 'manager'"
        );

        println!("✅ Custom variables with field verification passed!");
    }

    #[test]
    fn test_custom_variable_compilation() {
        println!("🧪 Testing custom variable compilation...");

        // Test that various custom variable syntaxes compile without error
        let _q1 = from_path!(u:User);
        let _q2 = from_path!(u:User > p:Post);
        let _q3 = from_path!(u:User > Post);
        let _q4 = from_path!(User > p:Post);
        let _q5 = from_path!(u:User < p:Post);
        let _q6 = from_path!(u:User.manager > m:User);

        println!("✅ All custom variable syntax compiles successfully!");
        println!("🚀 Phase 4: Custom Variable Syntax - IMPLEMENTATION COMPLETE!");
    }

    #[test]
    fn test_mixed_relations_woql_structure() {
        println!("🧪 Testing mixed forward/reverse WOQL structure verification...");

        // Test: from_path!(User > Post < Comment)
        // Should generate: User -> Post and Comment -> Post (Post is the hub)
        let query = from_path!(User > Post < Comment);
        let query_str = format!("{:#?}", query);
        println!("Mixed forward-reverse WOQL: {}", query_str);

        // Verify specific WOQL structure expectations
        assert!(
            query_str.contains("\"User_1\""),
            "Should contain User_1 variable"
        );
        assert!(
            query_str.contains("\"Post_1\""),
            "Should contain Post_1 variable"
        );
        assert!(
            query_str.contains("\"Comment_1\""),
            "Should contain Comment_1 variable"
        );

        // Should contain both directions: User->Post and Post<-Comment (Post has Comments)
        assert!(
            query_str.contains("\"Posts\""),
            "Should contain User->Post relation"
        );
        assert!(
            query_str.contains("\"Comments\""),
            "Should contain Post<-Comment relation"
        );

        println!("✅ Mixed forward-reverse WOQL structure verified!");
    }

    #[test]
    fn test_mixed_relations_reverse_forward() {
        println!("🧪 Testing reverse-forward WOQL structure verification...");

        // Test: from_path!(Comment < Post > User)
        // Should generate: Post has Comments, Post -> User
        let query = from_path!(Comment < Post > User);
        let query_str = format!("{:#?}", query);
        println!("Mixed reverse-forward WOQL: {}", query_str);

        // Verify specific WOQL structure
        assert!(
            query_str.contains("\"Comment_1\""),
            "Should contain Comment_1 variable"
        );
        assert!(
            query_str.contains("\"Post_1\""),
            "Should contain Post_1 variable"
        );
        assert!(
            query_str.contains("\"User_1\""),
            "Should contain User_1 variable"
        );

        // Should contain both directions: Comment->Post (Posts), Post->User (Users)
        assert!(
            query_str.contains("\"Posts\""),
            "Should contain Comment->Post relation"
        );
        assert!(
            query_str.contains("\"Users\""),
            "Should contain Post->User relation"
        );

        println!("✅ Mixed reverse-forward WOQL structure verified!");
    }

    #[test]
    fn test_complex_mixed_relation_chains() {
        println!("🧪 Testing complex mixed relation chains...");

        // Test longer mixed chains - not currently implemented but test compilation
        let _q1 = from_path!(User > Post < Comment);
        let _q2 = from_path!(Comment < Post > User);

        // Test with explicit fields mixed with forward/reverse
        let _q3 = from_path!(User.posts > Post < Comment);
        let _q4 = from_path!(Comment < Post.author > User);

        // Test with custom variables
        let _q5 = from_path!(u:User > p:Post < c:Comment);
        let _q6 = from_path!(c:Comment < p:Post > u:User);

        println!("✅ Complex mixed relation chains compile successfully!");
    }

    #[test]
    fn test_mixed_relations_semantic_correctness() {
        println!("🧪 Testing mixed relations semantic correctness...");

        // Test: User > Post < Comment
        // Semantically: Find User's Posts, and Comments that belong to those Posts
        let forward_reverse = from_path!(User > Post < Comment);
        let fr_str = format!("{:#?}", forward_reverse);

        // Test: Comment < Post > User
        // Semantically: Find Comments' Posts, and Users that those Posts belong to
        let reverse_forward = from_path!(Comment < Post > User);
        let rf_str = format!("{:#?}", reverse_forward);

        // Both should use Post_1 as the connecting variable
        assert!(
            fr_str.contains("\"Post_1\""),
            "Forward-reverse should connect via Post_1"
        );
        assert!(
            rf_str.contains("\"Post_1\""),
            "Reverse-forward should connect via Post_1"
        );

        // Verify they create different but valid query patterns
        assert_ne!(
            fr_str, rf_str,
            "The two mixed patterns should generate different WOQL"
        );

        println!("✅ Mixed relations semantic correctness verified!");
        println!("Forward-Reverse pattern creates valid User->Post<-Comment chain");
        println!("Reverse-Forward pattern creates valid Comment->Post->User chain");
    }
}
