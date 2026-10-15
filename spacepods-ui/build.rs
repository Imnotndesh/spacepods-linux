fn main() {
    // Rebuild the bundle when any drawing or the bundle list changes.
    println!("cargo:rerun-if-changed=resources");
    glib_build_tools::compile_resources(
        &["resources"],
        "resources/resources.gresource.xml",
        "spacepods.gresource",
    );
}
