use terminusdb_schema_derive::TerminusDBModel;
use terminusdb_schema::{TdbLazy, ToTDBInstance};
use terminusdb_relation::{RelationTo, RelationField};
use serde::{Serialize, Deserialize};
use terminusdb_woql2::prelude::Query;

// Import all WOQL macros that the generated code needs
use terminusdb_woql2::{triple, type_, var, and, optional};

// Required for TerminusDBModel derive to work
use terminusdb_schema as terminusdb_schema;

#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
#[tdb(key = "random", class_name = "User")]
struct User {
    id: String,
    name: String,
    posts: Vec<TdbLazy<Post>>,
    manager: Option<TdbLazy<User>>,
}

#[derive(TerminusDBModel, Serialize, Deserialize, Debug, Clone)]
#[tdb(key = "random", class_name = "Post")]
struct Post {
    id: String,
    title: String,
    author: TdbLazy<User>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_field_woql_generation() {
        println!("🧪 Testing WOQL generation for String field relations...");

        // Test User.name -> String relation (using unchecked method that derive macro generates)
        let query = <User as RelationTo<String, UserFields::Name>>::_constraints_with_vars_unchecked("user", "name");
        println!("User.name query: {:#?}", query);

        // Test Post.title -> String relation
        let query2 = <Post as RelationTo<String, PostFields::Title>>::_constraints_with_vars_unchecked("post", "title");
        println!("Post.title query: {:#?}", query2);

        println!("✅ String field WOQL generation works!");
    }

    #[test]
    fn test_model_field_woql_generation() {
        println!("🧪 Testing WOQL generation for TdbLazy<Model> field relations...");

        // Test Post.author -> TdbLazy<User> relation (using unchecked since TdbLazy doesn't implement TerminusDBModel)
        let query = <Post as RelationTo<TdbLazy<User>, PostFields::Author>>::_constraints_with_vars_unchecked("post", "author");
        println!("Post.author query: {:#?}", query);

        println!("✅ Model field WOQL generation works!");
    }

    #[test]
    fn test_vec_field_woql_generation() {
        println!("🧪 Testing WOQL generation for Vec<TdbLazy<Model>> field relations...");

        // Test User.posts -> Vec<TdbLazy<Post>> relation (using unchecked since Vec doesn't implement TerminusDBModel)
        let query = <User as RelationTo<Vec<TdbLazy<Post>>, UserFields::Posts>>::_constraints_with_vars_unchecked("user", "posts");
        println!("User.posts query: {:#?}", query);

        println!("✅ Vec field WOQL generation works!");
    }

    #[test]
    fn test_option_field_woql_generation() {
        println!("🧪 Testing WOQL generation for Option<TdbLazy<Model>> field relations...");

        // Test User.manager -> Option<TdbLazy<User>> relation (using unchecked since Option doesn't implement TerminusDBModel)
        let query = <User as RelationTo<Option<TdbLazy<User>>, UserFields::Manager>>::_constraints_with_vars_unchecked("user", "manager");
        println!("User.manager query: {:#?}", query);

        println!("✅ Option field WOQL generation works!");
    }

    #[test]
    fn test_woql_structure_validation() {
        println!("🔍 Validating WOQL query structure...");

        // Generate a query and examine its structure (using unchecked for String field)
        let query = <User as RelationTo<String, UserFields::Name>>::_constraints_with_vars_unchecked("u", "n");

        // The query should be an And query with triple, type constraints
        match query {
            Query::And(_) => {
                println!("✅ Query is properly structured as And clause");
            },
            _ => {
                println!("❌ Unexpected query structure: {:#?}", query);
                panic!("Query should be an And clause");
            }
        }

        println!("✅ WOQL structure validation passed!");
    }

    #[test]
    fn test_all_relation_types_compile_and_run() {
        println!("🚀 Testing that ALL generated relation types can execute WOQL generation...");

        // String fields (using unchecked methods that derive macro generates)
        let _q1 = <User as RelationTo<String, UserFields::Id>>::_constraints_with_vars_unchecked("u", "id");
        let _q2 = <User as RelationTo<String, UserFields::Name>>::_constraints_with_vars_unchecked("u", "name");
        let _q3 = <Post as RelationTo<String, PostFields::Id>>::_constraints_with_vars_unchecked("p", "id");
        let _q4 = <Post as RelationTo<String, PostFields::Title>>::_constraints_with_vars_unchecked("p", "title");
        println!("✅ All String field relations generate WOQL");

        // TdbLazy fields (using unchecked since TdbLazy doesn't implement TerminusDBModel)
        let _q5 = <Post as RelationTo<TdbLazy<User>, PostFields::Author>>::_constraints_with_vars_unchecked("p", "author");
        println!("✅ All TdbLazy field relations generate WOQL");

        // Vec<TdbLazy> fields (using unchecked since Vec doesn't implement TerminusDBModel)
        let _q6 = <User as RelationTo<Vec<TdbLazy<Post>>, UserFields::Posts>>::_constraints_with_vars_unchecked("u", "posts");
        println!("✅ All Vec<TdbLazy> field relations generate WOQL");

        // Option<TdbLazy> fields (using unchecked since Option doesn't implement TerminusDBModel)
        let _q7 = <User as RelationTo<Option<TdbLazy<User>>, UserFields::Manager>>::_constraints_with_vars_unchecked("u", "manager");
        println!("✅ All Option<TdbLazy> field relations generate WOQL");

        println!("🎉 COMPLETE SUCCESS: All relation types generate working WOQL!");
    }

    #[test]
    fn test_variable_names_in_generated_woql() {
        println!("🔍 Testing that custom variable names are used in generated WOQL...");

        let query = <User as RelationTo<String, UserFields::Name>>::_constraints_with_vars_unchecked("custom_user", "custom_name");
        println!("Query with custom vars: {:#?}", query);

        // The query should contain our custom variable names
        let query_str = format!("{:#?}", query);
        assert!(query_str.contains("custom_user"), "Query should contain custom source variable");
        assert!(query_str.contains("custom_name"), "Query should contain custom target variable");

        println!("✅ Custom variable names are properly used in generated WOQL!");
    }

    #[test]
    fn test_field_names_in_generated_woql() {
        println!("🔍 Testing that field names are correctly used in generated WOQL...");

        let query = <User as RelationTo<String, UserFields::Name>>::_constraints_with_vars_unchecked("u", "n");
        println!("User.name query: {:#?}", query);

        // The query should contain the field name "name"
        let query_str = format!("{:#?}", query);
        assert!(query_str.contains("name"), "Query should contain the field name 'name'");

        let query2 = <Post as RelationTo<String, PostFields::Title>>::_constraints_with_vars_unchecked("p", "t");
        let query2_str = format!("{:#?}", query2);
        assert!(query2_str.contains("title"), "Query should contain the field name 'title'");

        println!("✅ Field names are correctly embedded in generated WOQL!");
    }
}