fn main() {
    // Compile GResources
    glib_build_tools::compile_resources(
        &["resources"],
        "resources/resources.gresource.xml",
        "vmod.gresource",
    );

    // Compile GSettings schema for development
    println!("cargo:rerun-if-changed=resources/org.vmod.VMOD.gschema.xml");

    let status = std::process::Command::new("glib-compile-schemas")
        .arg("resources/")
        .status();

    if let Ok(status) = status {
        if !status.success() {
            eprintln!("Warning: Failed to compile GSettings schema");
        }
    } else {
        eprintln!("Warning: glib-compile-schemas not found, schema not compiled");
    }
}
