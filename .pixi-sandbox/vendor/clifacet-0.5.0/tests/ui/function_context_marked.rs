struct App;

// The marker exists for a context whose name is the application's to choose; a value type
// marked as the context is a mistake about what the context is.
#[clifacet::command]
fn add(#[clifacet(context)] store: String) -> Result<(), clifacet::Failure> {
    let _ = store;
    Ok(())
}

fn main() {}
