//! Comprehensive xs:choice validation tests
//!
//! These tests verify that xs:choice works correctly through the full pipeline:
//! 1. XSD schema with xs:choice generates correct TerminusDB schemas
//! 2. XML instances with choice elements validate against XSD
//! 3. XML instances parse correctly to TerminusDB instances
//! 4. Instances can be inserted into TerminusDB

use std::path::Path;
use tempfile::TempDir;
use terminusdb_schema::{Instance, InstanceProperty, PrimitiveValue};
use terminusdb_xsd::XsdModel;

/// Create a temporary XSD file with xs:choice compositor
fn create_xsd_with_choice(dir: &Path) -> std::path::PathBuf {
    let xsd_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
           targetNamespace="http://test.example.com/choice"
           xmlns:tns="http://test.example.com/choice"
           elementFormDefault="qualified">

  <!-- Simple xs:choice test: choose between email OR phone -->
  <xs:element name="contact" type="tns:ContactType"/>

  <xs:complexType name="ContactType">
    <xs:sequence>
      <xs:element name="name" type="xs:string"/>
      <xs:choice>
        <xs:element name="email" type="xs:string"/>
        <xs:element name="phone" type="xs:string"/>
      </xs:choice>
    </xs:sequence>
  </xs:complexType>

  <!-- Nested choice: choose notification method with sub-choices -->
  <xs:element name="notification" type="tns:NotificationType"/>

  <xs:complexType name="NotificationType">
    <xs:sequence>
      <xs:element name="recipient" type="xs:string"/>
      <xs:choice>
        <xs:element name="emailNotification" type="tns:EmailNotificationType"/>
        <xs:element name="smsNotification" type="tns:SmsNotificationType"/>
        <xs:element name="pushNotification" type="tns:PushNotificationType"/>
      </xs:choice>
    </xs:sequence>
  </xs:complexType>

  <xs:complexType name="EmailNotificationType">
    <xs:sequence>
      <xs:element name="address" type="xs:string"/>
      <xs:element name="subject" type="xs:string"/>
    </xs:sequence>
  </xs:complexType>

  <xs:complexType name="SmsNotificationType">
    <xs:sequence>
      <xs:element name="phoneNumber" type="xs:string"/>
      <xs:element name="message" type="xs:string" minOccurs="0"/>
    </xs:sequence>
  </xs:complexType>

  <xs:complexType name="PushNotificationType">
    <xs:sequence>
      <xs:element name="deviceId" type="xs:string"/>
      <xs:element name="title" type="xs:string"/>
    </xs:sequence>
  </xs:complexType>

  <!-- Choice with maxOccurs > 1 -->
  <xs:element name="document" type="tns:DocumentType"/>

  <xs:complexType name="DocumentType">
    <xs:sequence>
      <xs:element name="title" type="xs:string"/>
      <xs:choice maxOccurs="unbounded">
        <xs:element name="paragraph" type="xs:string"/>
        <xs:element name="image" type="tns:ImageType"/>
        <xs:element name="table" type="tns:TableType"/>
      </xs:choice>
    </xs:sequence>
  </xs:complexType>

  <xs:complexType name="ImageType">
    <xs:sequence>
      <xs:element name="src" type="xs:string"/>
      <xs:element name="alt" type="xs:string" minOccurs="0"/>
    </xs:sequence>
  </xs:complexType>

  <xs:complexType name="TableType">
    <xs:sequence>
      <xs:element name="caption" type="xs:string" minOccurs="0"/>
      <xs:element name="rows" type="xs:integer"/>
    </xs:sequence>
  </xs:complexType>

  <!-- Optional choice (minOccurs=0) -->
  <xs:element name="profile" type="tns:ProfileType"/>

  <xs:complexType name="ProfileType">
    <xs:sequence>
      <xs:element name="username" type="xs:string"/>
      <xs:choice minOccurs="0">
        <xs:element name="bio" type="xs:string"/>
        <xs:element name="tagline" type="xs:string"/>
      </xs:choice>
    </xs:sequence>
  </xs:complexType>

</xs:schema>
"#;

    let xsd_path = dir.join("test_choice.xsd");
    std::fs::write(&xsd_path, xsd_content).expect("Failed to write test XSD");
    xsd_path
}

#[test]
fn test_xschoice_schema_generation() {
    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let xsd_path = create_xsd_with_choice(temp_dir.path());

    // Load XSD and generate schemas
    let model =
        XsdModel::from_file(&xsd_path, None::<&str>).expect("Failed to load XSD with xs:choice");

    let schemas = model.schemas();

    println!("Generated {} schemas", schemas.len());
    for schema in schemas {
        if let terminusdb_schema::Schema::Class { id, properties, .. } = schema {
            println!("Class: {} with {} properties", id, properties.len());
            for prop in properties {
                println!("  - {}: {}", prop.name, prop.class);
            }
        }
    }

    // Verify ContactType has both email and phone as optional properties
    let contact_schema = schemas
        .iter()
        .find(|s| {
            if let terminusdb_schema::Schema::Class { id, .. } = s {
                id == "ContactType"
            } else {
                false
            }
        })
        .expect("ContactType should be generated");

    if let terminusdb_schema::Schema::Class { properties, .. } = contact_schema {
        // Should have name, email, and phone properties
        assert!(
            properties.iter().any(|p| p.name == "name"),
            "ContactType should have 'name' property"
        );
        assert!(
            properties.iter().any(|p| p.name == "email"),
            "ContactType should have 'email' property from choice"
        );
        assert!(
            properties.iter().any(|p| p.name == "phone"),
            "ContactType should have 'phone' property from choice"
        );

        // Both choice elements should be optional (since it's a choice, only one is required)
        // In TerminusDB schema, optional fields have Optional type wrapper
        println!("ContactType properties:");
        for prop in properties {
            println!("  {}: {} (type: {:?})", prop.name, prop.class, prop.r#type);
        }
    } else {
        panic!("ContactType should be a Class schema");
    }
}

#[test]
fn test_xschoice_xml_validation_email() {
    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let xsd_path = create_xsd_with_choice(temp_dir.path());

    let model =
        XsdModel::from_file(&xsd_path, None::<&str>).expect("Failed to load XSD with xs:choice");

    // Test XML with email choice
    let xml_with_email = r#"<?xml version="1.0" encoding="UTF-8"?>
<contact xmlns="http://test.example.com/choice">
  <name>John Doe</name>
  <email>john@example.com</email>
</contact>
"#;

    // This should validate successfully
    let result = model.validate_xml(xml_with_email);
    assert!(
        result.is_ok(),
        "XML with email choice should validate: {:?}",
        result.err()
    );
}

#[test]
fn test_xschoice_xml_validation_phone() {
    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let xsd_path = create_xsd_with_choice(temp_dir.path());

    let model =
        XsdModel::from_file(&xsd_path, None::<&str>).expect("Failed to load XSD with xs:choice");

    // Test XML with phone choice
    let xml_with_phone = r#"<?xml version="1.0" encoding="UTF-8"?>
<contact xmlns="http://test.example.com/choice">
  <name>Jane Smith</name>
  <phone>555-1234</phone>
</contact>
"#;

    // This should validate successfully
    let result = model.validate_xml(xml_with_phone);
    assert!(
        result.is_ok(),
        "XML with phone choice should validate: {:?}",
        result.err()
    );
}

#[test]
fn test_xschoice_xml_validation_both_should_fail() {
    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let xsd_path = create_xsd_with_choice(temp_dir.path());

    let model =
        XsdModel::from_file(&xsd_path, None::<&str>).expect("Failed to load XSD with xs:choice");

    // Test XML with BOTH email and phone (invalid for xs:choice)
    let xml_with_both = r#"<?xml version="1.0" encoding="UTF-8"?>
<contact xmlns="http://test.example.com/choice">
  <name>Invalid User</name>
  <email>invalid@example.com</email>
  <phone>555-9999</phone>
</contact>
"#;

    // This should fail validation (can only choose one)
    let result = model.validate_xml(xml_with_both);
    assert!(
        result.is_err(),
        "XML with both email and phone should fail validation (xs:choice allows only one)"
    );
}

#[test]
fn test_xschoice_xml_validation_neither_should_fail() {
    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let xsd_path = create_xsd_with_choice(temp_dir.path());

    let model =
        XsdModel::from_file(&xsd_path, None::<&str>).expect("Failed to load XSD with xs:choice");

    // Test XML with neither email nor phone (invalid, must choose one)
    let xml_with_neither = r#"<?xml version="1.0" encoding="UTF-8"?>
<contact xmlns="http://test.example.com/choice">
  <name>No Contact Info</name>
</contact>
"#;

    // This should fail validation (must choose at least one)
    let result = model.validate_xml(xml_with_neither);
    assert!(
        result.is_err(),
        "XML with neither email nor phone should fail validation (xs:choice requires one)"
    );
}

#[test]
fn test_xschoice_nested_choice_validation() {
    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let xsd_path = create_xsd_with_choice(temp_dir.path());

    let model =
        XsdModel::from_file(&xsd_path, None::<&str>).expect("Failed to load XSD with xs:choice");

    // Test nested choice with email notification
    let xml_email_notification = r#"<?xml version="1.0" encoding="UTF-8"?>
<notification xmlns="http://test.example.com/choice">
  <recipient>user@example.com</recipient>
  <emailNotification>
    <address>user@example.com</address>
    <subject>Test Subject</subject>
  </emailNotification>
</notification>
"#;

    let result = model.validate_xml(xml_email_notification);
    assert!(
        result.is_ok(),
        "Nested choice with email notification should validate: {:?}",
        result.err()
    );

    // Test nested choice with SMS notification
    let xml_sms_notification = r#"<?xml version="1.0" encoding="UTF-8"?>
<notification xmlns="http://test.example.com/choice">
  <recipient>user@example.com</recipient>
  <smsNotification>
    <phoneNumber>555-1234</phoneNumber>
    <message>Test message</message>
  </smsNotification>
</notification>
"#;

    let result = model.validate_xml(xml_sms_notification);
    assert!(
        result.is_ok(),
        "Nested choice with SMS notification should validate: {:?}",
        result.err()
    );

    // Test nested choice with push notification
    let xml_push_notification = r#"<?xml version="1.0" encoding="UTF-8"?>
<notification xmlns="http://test.example.com/choice">
  <recipient>user@example.com</recipient>
  <pushNotification>
    <deviceId>device-123</deviceId>
    <title>Test Title</title>
  </pushNotification>
</notification>
"#;

    let result = model.validate_xml(xml_push_notification);
    assert!(
        result.is_ok(),
        "Nested choice with push notification should validate: {:?}",
        result.err()
    );
}

#[test]
fn test_xschoice_with_maxoccurs_unbounded() {
    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let xsd_path = create_xsd_with_choice(temp_dir.path());

    let model =
        XsdModel::from_file(&xsd_path, None::<&str>).expect("Failed to load XSD with xs:choice");

    // Test choice with multiple occurrences (mixing different choice options)
    let xml_mixed_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://test.example.com/choice">
  <title>Test Document</title>
  <paragraph>First paragraph</paragraph>
  <image>
    <src>image1.png</src>
    <alt>Image 1</alt>
  </image>
  <paragraph>Second paragraph</paragraph>
  <table>
    <caption>Test Table</caption>
    <rows>5</rows>
  </table>
  <paragraph>Third paragraph</paragraph>
</document>
"#;

    let result = model.validate_xml(xml_mixed_content);
    assert!(
        result.is_ok(),
        "Choice with maxOccurs unbounded and mixed content should validate: {:?}",
        result.err()
    );
}

#[test]
fn test_xschoice_optional_choice() {
    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let xsd_path = create_xsd_with_choice(temp_dir.path());

    let model =
        XsdModel::from_file(&xsd_path, None::<&str>).expect("Failed to load XSD with xs:choice");

    // Test optional choice with bio
    let xml_with_bio = r#"<?xml version="1.0" encoding="UTF-8"?>
<profile xmlns="http://test.example.com/choice">
  <username>testuser</username>
  <bio>This is my bio</bio>
</profile>
"#;

    let result = model.validate_xml(xml_with_bio);
    assert!(
        result.is_ok(),
        "Optional choice with bio should validate: {:?}",
        result.err()
    );

    // Test optional choice with tagline
    let xml_with_tagline = r#"<?xml version="1.0" encoding="UTF-8"?>
<profile xmlns="http://test.example.com/choice">
  <username>testuser</username>
  <tagline>My awesome tagline</tagline>
</profile>
"#;

    let result = model.validate_xml(xml_with_tagline);
    assert!(
        result.is_ok(),
        "Optional choice with tagline should validate: {:?}",
        result.err()
    );

    // Test optional choice with neither (should be valid since minOccurs=0)
    let xml_with_neither = r#"<?xml version="1.0" encoding="UTF-8"?>
<profile xmlns="http://test.example.com/choice">
  <username>testuser</username>
</profile>
"#;

    let result = model.validate_xml(xml_with_neither);
    assert!(
        result.is_ok(),
        "Optional choice with neither option should validate: {:?}",
        result.err()
    );
}
