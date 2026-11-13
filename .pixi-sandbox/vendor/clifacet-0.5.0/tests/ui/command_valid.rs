//! The wrapper compiles a command and bridges it to typed input.
fn main() {
    use clifacet::Facet;

    #[clifacet::command(input = Greeting)]
    #[command(name = "greet", about = "Say hello")]
    struct Cli {
        #[arg(long, alias = "nickname", short = 'n')]
        name: Option<String>,
    }

    /// The typed input the bridge builds field by field.
    struct Greeting {
        name: Option<String>,
    }

    let cli = <Cli as clifacet::clap::Parser>::try_parse_from(["greet", "--nickname", "ada"])
        .expect("an alias parses");
    let _ = <Cli as Facet>::command();
    let greeted = cli.validate().expect("valid");
    assert_eq!(greeted.name.as_deref(), Some("ada"));
}
