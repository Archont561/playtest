struct App;

#[clifacet::command]
fn add(
    cx: &App,
    #[clifacet(long, help = "The record")] record: String,
) -> Result<(), clifacet::Failure> {
    let _ = (cx, record);
    Ok(())
}

fn main() {}
