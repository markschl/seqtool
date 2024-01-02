use std::fs::{read_to_string, File};
use std::io::{BufWriter, Write};
use std::path::Path;

fn main() {
    #[cfg(feature = "expr")]
    write_js();
}

#[cfg_attr(not(feature = "expr"), allow(unused))]
fn write_js() {
    let js_include = read_to_string(Path::new("js").join("include.js")).unwrap();

    let js_include = regex::Regex::new(r"(\s+|\n)")
        .unwrap()
        .replace_all(&js_include, " ");

    let path = Path::new("src")
        .join("var")
        .join("modules")
        .join("expr")
        .join("_js_include.rs");
    let mut out = BufWriter::new(File::create(path).unwrap());

    writeln!(&mut out, "static JS_INCLUDE: &str = r#\"{}\"#;", js_include).unwrap();
}
