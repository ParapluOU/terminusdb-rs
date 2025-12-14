#![cfg(feature = "generic-derive")]

use terminusdb_schema::ToSchemaProperty;
use terminusdb_schema::{EntityIDFor, ToTDBInstance, ToTDBSchema};
use terminusdb_schema_derive::TerminusDBModel;

// Define concrete TerminusDBModel types
#[derive(Debug, Clone, TerminusDBModel)]
struct Task {
    id: String,
    title: String,
    completed: bool,
}

#[derive(Debug, Clone, TerminusDBModel)]
struct Project {
    id: String,
    name: String,
    description: String,
}

// The exact syntax requested: Model<T> { other: EntityIDFor<T> }
#[derive(Debug, Clone, TerminusDBModel)]
struct Reference<T>
where
    T: ToTDBSchema + terminusdb_schema::ToSchemaClass + Send,
{
    id: String,
    referenced_id: EntityIDFor<T>,
    notes: String,
}

#[test]
fn test_reference_with_models() {
    // Create a reference to a Task
    let task_ref = Reference::<Task> {
        id: "ref-task-1".to_string(),
        referenced_id: EntityIDFor::new("task-001").unwrap(),
        notes: "Important task reference".to_string(),
    };

    // Verify we can get the schema
    let task_schema = <Reference<Task> as ToTDBSchema>::to_schema();
    assert_eq!(task_schema.class_name(), "Reference<Task>");

    // Create a reference to a Project
    let project_ref = Reference::<Project> {
        id: "ref-proj-1".to_string(),
        referenced_id: EntityIDFor::new("project-001").unwrap(),
        notes: "Main project reference".to_string(),
    };

    let project_schema = <Reference<Project> as ToTDBSchema>::to_schema();
    assert_eq!(project_schema.class_name(), "Reference<Project>");

    // Verify we can convert to instances
    let _task_instance = task_ref.to_instance(None);
    let _project_instance = project_ref.to_instance(None);

    println!("✅ Reference<T> {{ EntityIDFor<T> }} pattern works perfectly!");
}
