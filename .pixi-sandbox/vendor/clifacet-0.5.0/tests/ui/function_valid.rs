//! What a function command accepts, which is the shell the error cases are compared against.

/// Count the words.
///
/// One line per file, and the total at the end.
#[clifacet::command]
fn words(
    #[clifacet(long, help = "Only this many files")] limit: Option<usize>,
    files: Vec<String>,
) -> Result<usize, clifacet::Failure> {
    Ok(limit.map_or(files.len(), |limit| files.len().min(limit)))
}

fn main() {}
