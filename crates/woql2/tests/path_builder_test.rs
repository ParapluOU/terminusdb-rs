use terminusdb_woql2::path_builder::PathStart;
use terminusdb_woql2::query::Query;

#[cfg(test)]
mod tests {
    use super::*;

    // Test types
    struct User;
    struct Post;
    struct Comment;

    #[test]
    fn test_single_node() {
        println!("🧪 Testing single node builder...");

        let query = PathStart::new().node::<User>().finalize();

        match query {
            Query::Triple(_) => {
                println!("✅ Single node generates Triple");
            }
            _ => {
                panic!("Single node should generate Triple query");
            }
        }

        let query_str = format!("{:#?}", query);
        println!("Single node query: {}", query_str);
        assert!(query_str.contains("User"), "Should contain User type");
    }

    #[test]
    fn test_forward_relation() {
        println!("🧪 Testing forward relation builder...");

        let query = PathStart::new()
            .node::<User>()
            .forward()
            .node::<Post>()
            .finalize();

        match query {
            Query::And(_) => {
                println!("✅ Forward relation generates And query");
            }
            _ => {
                panic!("Forward relation should generate And query");
            }
        }

        let query_str = format!("{:#?}", query);
        println!("Forward relation query: {}", query_str);
        assert!(query_str.contains("User"), "Should contain User type");
        assert!(query_str.contains("Post"), "Should contain Post type");
    }

    #[test]
    fn test_backward_relation() {
        println!("🧪 Testing backward relation builder...");

        let query = PathStart::new()
            .node::<Comment>()
            .backward()
            .node::<Post>()
            .finalize();

        match query {
            Query::And(_) => {
                println!("✅ Backward relation generates And query");
            }
            _ => {
                panic!("Backward relation should generate And query");
            }
        }

        let query_str = format!("{:#?}", query);
        println!("Backward relation query: {}", query_str);
        assert!(query_str.contains("Comment"), "Should contain Comment type");
        assert!(query_str.contains("Post"), "Should contain Post type");
    }

    #[test]
    fn test_explicit_field() {
        println!("🧪 Testing explicit field builder...");

        let query = PathStart::new()
            .node::<User>()
            .field("manager")
            .node::<User>()
            .finalize();

        match query {
            Query::And(_) => {
                println!("✅ Explicit field generates And query");
            }
            _ => {
                panic!("Explicit field should generate And query");
            }
        }

        let query_str = format!("{:#?}", query);
        println!("Explicit field query: {}", query_str);
        assert!(
            query_str.contains("manager"),
            "Should contain field name 'manager'"
        );
    }

    #[test]
    fn test_long_chain() {
        println!("🧪 Testing long chain builder...");

        let query = PathStart::new()
            .node::<User>()
            .forward()
            .node::<Post>()
            .forward()
            .node::<Comment>()
            .forward()
            .node::<User>() // Comment author
            .finalize();

        match query {
            Query::And(_) => {
                println!("✅ Long chain generates And query");
            }
            _ => {
                panic!("Long chain should generate And query");
            }
        }

        let query_str = format!("{:#?}", query);
        println!("Long chain query: {}", query_str);

        // Should contain all types
        assert!(query_str.contains("User"), "Should contain User type");
        assert!(query_str.contains("Post"), "Should contain Post type");
        assert!(query_str.contains("Comment"), "Should contain Comment type");

        println!("🎉 Long chain builder works - unlimited length possible!");
    }

    #[test]
    fn test_mixed_directions() {
        println!("🧪 Testing mixed forward/backward directions...");

        let query = PathStart::new()
            .node::<User>()
            .forward()
            .node::<Post>()
            .backward() // Post has Comments
            .node::<Comment>()
            .finalize();

        match query {
            Query::And(_) => {
                println!("✅ Mixed directions generate And query");
            }
            _ => {
                panic!("Mixed directions should generate And query");
            }
        }

        let query_str = format!("{:#?}", query);
        println!("Mixed directions query: {}", query_str);

        assert!(query_str.contains("User"), "Should contain User type");
        assert!(query_str.contains("Post"), "Should contain Post type");
        assert!(query_str.contains("Comment"), "Should contain Comment type");
    }

    #[test]
    fn test_custom_variables() {
        println!("🧪 Testing custom variable names...");

        let query = PathStart::new()
            .variable::<User>("u")
            .forward()
            .variable::<Post>("p")
            .finalize();

        match query {
            Query::And(_) => {
                println!("✅ Custom variables generate And query");
            }
            _ => {
                panic!("Custom variables should generate And query");
            }
        }

        let query_str = format!("{:#?}", query);
        println!("Custom variables query: {}", query_str);

        // Should contain custom variable names
        assert!(
            query_str.contains("\"u\""),
            "Should contain custom variable 'u'"
        );
        assert!(
            query_str.contains("\"p\""),
            "Should contain custom variable 'p'"
        );
        assert!(
            !query_str.contains("User_1"),
            "Should NOT contain auto-generated User_1"
        );
        assert!(
            !query_str.contains("Post_1"),
            "Should NOT contain auto-generated Post_1"
        );
    }

    #[test]
    fn test_mixed_custom_auto_variables() {
        println!("🧪 Testing mixed custom and auto variables...");

        let query = PathStart::new()
            .variable::<User>("u")
            .forward()
            .node::<Post>() // Auto-generated
            .forward()
            .variable::<Comment>("c") // Custom
            .finalize();

        let query_str = format!("{:#?}", query);
        println!("Mixed variables query: {}", query_str);

        // Should contain both custom and auto-generated
        assert!(
            query_str.contains("\"u\""),
            "Should contain custom variable 'u'"
        );
        assert!(
            query_str.contains("\"c\""),
            "Should contain custom variable 'c'"
        );
        assert!(
            query_str.contains("Post"),
            "Should contain auto-generated Post variable"
        );
    }

    #[test]
    fn test_variable_access() {
        println!("🧪 Testing variable name access...");

        let path = PathStart::new()
            .variable::<User>("u")
            .forward()
            .variable::<Post>("p");

        // Test variable access
        assert_eq!(path.source_variable(), "u", "Source variable should be 'u'");
        assert_eq!(path.final_variable(), "p", "Final variable should be 'p'");

        println!("✅ Variable access methods work correctly");
    }
}
