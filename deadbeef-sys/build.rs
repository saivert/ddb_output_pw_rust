extern crate bindgen;
use std::env;
use std::path::PathBuf;

fn main() {
    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=wrapper.h");

    const INCLUDED_TYPES: &[&str] = &[
        "ddb_.*",
        "playback_order_t",
        "playback_mode_t",
        "ddb_plugin_flag_t",
        "db_log_layer_t",
        "db_plugin_type_t",
        "db_ev_t",
        "DB_.*"
    ];
    const INCLUDED_VARS: &[&str] = &[
        "DB_.*",
        "DDB_.*"
    ];

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let mut builder = bindgen::Builder::default();
    builder = builder.header("wrapper.h")
                .rustfmt_bindings(true)
                .derive_default(true)
                .derive_eq(true)
                .default_enum_style(bindgen::EnumVariation::Consts)
                .prepend_enum_name(false);

    for t in INCLUDED_TYPES {
        builder = builder.allowlist_type(t);
    }
    
    for v in INCLUDED_VARS {
        builder = builder.allowlist_var(v);
    }

    // Tell cargo to invalidate the built crate whenever any of the
    // included header files changed.
    builder = builder.parse_callbacks(Box::new(bindgen::CargoCallbacks));

    // Finish the builder and generate the bindings.
    let bindings = builder.generate().expect("Unable to generate bindings");
    
    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

}