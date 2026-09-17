use runtimepack_core::build_plan::generator::AndroidProjectGenerator;
use runtimepack_core::diagnostics::DiagnosticCode;

#[test]
fn test_forbidden_storage_permissions_rejected() {
    let forbidden = vec![
        "android.permission.INTERNET".to_string(),
        "android.permission.MANAGE_EXTERNAL_STORAGE".to_string(),
    ];
    let res = AndroidProjectGenerator::generate_permissions_xml(&forbidden);
    assert!(res.is_err());
    let err = res.unwrap_err();
    if let runtimepack_core::diagnostics::DiagnosticError::Detailed { code, .. } = err {
        assert_eq!(code.0, DiagnosticCode::AND_FORBIDDEN_PERMISSION);
    } else {
        panic!("Expected DiagnosticError::Detailed");
    }
}

#[test]
fn test_allowed_permissions_xml() {
    let allowed = vec!["android.permission.INTERNET".to_string()];
    let res = AndroidProjectGenerator::generate_permissions_xml(&allowed);
    assert!(res.is_ok());
    let xml = res.unwrap();
    assert!(xml.contains("android.permission.INTERNET"));
    assert!(!xml.contains("MANAGE_EXTERNAL_STORAGE"));
}

#[test]
fn test_generate_manifest_xml() {
    let xml = AndroidProjectGenerator::generate_manifest_xml(
        "com.example.testgame",
        "Test Game",
        "sensorLandscape",
        &["android.permission.INTERNET".to_string()],
    ).unwrap();

    assert!(xml.contains("package=\"com.example.testgame\""));
    assert!(xml.contains("android:label=\"Test Game\""));
    assert!(xml.contains("android:name=\"io.runtimepack.host.BootstrapActivity\""));
    assert!(xml.contains("android:name=\"io.runtimepack.host.RuntimeApplication\""));
}
