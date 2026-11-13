//! `input = Type` names the input with any type the generated impls can name.
//!
//! The derive writes the input type into `type Input`, the generated `TryFrom` and
//! `validate`'s signature, so every spelling below has to mean the same type in all of
//! them. Each command here parses real argv and validates through `Facet`, the same seam
//! an application uses: compiling proves the type was accepted, running proves the
//! generated bridge built it.

use std::marker::PhantomData;

use clifacet::Facet;
use clifacet::clap::Parser as _;
use rstest::rstest;

/// The value every command below is given.
const NAME: &str = "ada";

mod inputs {
    /// An input reached through a qualified path.
    pub struct Greeting {
        pub name: Option<String>,
    }
}

/// A command whose input is named by its full path.
#[clifacet::command(input = crate::inputs::Greeting)]
struct Qualified {
    #[clifacet(long)]
    name: Option<String>,
}

/// An input generic over the value it carries.
struct Labelled<T> {
    name: T,
}

/// A command whose input is a generic type with its argument written out.
#[clifacet::command(input = Labelled<String>)]
struct Generic {
    #[clifacet(long, fill = "world")]
    name: Option<String>,
}

/// An input generic over a lifetime, as one that borrows would be.
struct Borrowing<'a> {
    name: Option<String>,
    marker: PhantomData<&'a ()>,
}

/// A command declaring the lifetime its input uses: clap's derive carries it, and the
/// generated bridge copies both fields into the input by name.
#[clifacet::command(input = Borrowing<'a>)]
struct WithLifetime<'a> {
    #[clifacet(long)]
    name: Option<String>,

    #[arg(skip)]
    marker: PhantomData<&'a ()>,
}

/// A family of commands, each naming its input through a trait.
trait Shape {
    type Input;
}

/// The command family's marker.
struct Marker;

/// The input [`Marker`]'s shape names.
struct Projected {
    name: Option<String>,
}

impl Shape for Marker {
    type Input = Projected;
}

/// A command whose input is an associated type, built field by field through it.
#[clifacet::command(input = <Marker as Shape>::Input)]
struct Associated {
    #[clifacet(long)]
    name: Option<String>,
}

/// An alias, which names the struct it stands for.
type Alias = inputs::Greeting;

/// A command whose input is an alias.
#[clifacet::command(input = Alias)]
struct Aliased {
    #[clifacet(long)]
    name: Option<String>,
}

/// Parses `--name ada` with `Cli` and validates it, as an application would.
fn validated<Cli: Facet + clifacet::clap::Parser>() -> Cli::Input {
    let argv = ["test", "--name", NAME];
    Cli::try_parse_from(argv)
        .expect("the argv parses")
        .validate()
        .expect("the input is valid")
}

#[rstest]
#[case::qualified_path(validated::<Qualified>().name)]
#[case::alias(validated::<Aliased>().name)]
#[case::associated_type(validated::<Associated>().name)]
fn a_struct_path_however_it_is_spelled_is_built_field_by_field(#[case] name: Option<String>) {
    assert_eq!(name.as_deref(), Some(NAME));
}

#[test]
fn a_generic_input_is_built_with_its_field_rules() {
    assert_eq!(validated::<Generic>().name, NAME);

    let filled = Generic::try_parse_from(["test"])
        .expect("the argv parses")
        .validate()
        .expect("the input is valid");
    assert_eq!(filled.name, "world");
}

#[test]
fn an_input_generic_over_a_lifetime_is_built_through_the_commands_own() {
    let input = validated::<WithLifetime<'_>>();

    assert_eq!(input.name.as_deref(), Some(NAME));
    assert_eq!(input.marker, PhantomData);
}
