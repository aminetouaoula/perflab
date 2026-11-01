use clap::Parser;
#[derive(Parser)]
struct Cli {
    name: String,
}
fn main() {
    let cli = Cli::parse();
    println!("cli : {}", cli.name);
}
