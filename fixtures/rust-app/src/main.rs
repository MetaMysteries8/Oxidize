// Detection fixture only: automated Rust optimization is not implemented yet.
fn main() {
    let values: Vec<String> = (0..1000).map(|n| n.to_string()).collect();
    let joined = values.clone().join(",");
    println!("{}", joined.len());
}
