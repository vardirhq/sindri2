fn main() {
    let source = "#menu { width: 420px; }";
    let sheet = weave::parse(source).expect("valid Weave");
    println!("{} rule(s)", sheet.rules.len());
}
