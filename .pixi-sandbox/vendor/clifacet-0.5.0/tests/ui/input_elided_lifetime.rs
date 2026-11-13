use clifacet::Failure;

#[clifacet::command(input = Greeting<'_>)]
struct Cli {
    #[clifacet(long)]
    name: Option<String>,
}

struct Greeting<'a> {
    name: Option<String>,
    marker: std::marker::PhantomData<&'a ()>,
}

fn build(_cli: Cli) -> Result<(String, u8), Failure> {
    Ok((String::new(), 0))
}

fn main() {}
