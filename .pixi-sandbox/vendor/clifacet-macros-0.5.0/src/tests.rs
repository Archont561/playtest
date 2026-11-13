//! Unit tests for the derive's expansion and its diagnostics.
//!
//! `trybuild` proves what a *user* sees when something is wrong, but it compiles
//! each case in a separate process, so those runs cannot count toward this crate's
//! coverage and they cannot reach a branch in isolation. These tests call
//! [`expand`](crate::facet::expand) directly: one case per accept and per reject
//! rule, asserting the generated code or the exact message.

use syn::DeriveInput;

use crate::command::expand as expand_command;
use crate::facet::expand;

/// Expands a derive on the given source, as the macro would.
fn expanded(source: &str) -> syn::Result<String> {
    let input: DeriveInput = syn::parse_str(source).expect("the test source parses");
    expand(&input).map(|tokens| tokens.to_string())
}

/// Expands a derive that must fail, returning the reported message.
fn rejected(source: &str) -> String {
    expanded(source)
        .err()
        .map(|error| error.to_string())
        .expect("this derive must be rejected")
}

#[test]
fn a_declared_input_is_built_field_by_field() {
    let tokens = expanded(
        r"
        #[facet(input = Greeting)]
        struct Cli {
            name: Option<String>,
            repeat: u8,
        }
        ",
    )
    .expect("a named struct with an input type is supported");

    assert!(
        tokens.contains("impl :: clifacet :: Facet for Cli"),
        "{tokens}"
    );
    assert!(tokens.contains("type Input = Greeting"), "{tokens}");
    // The copy lives in the generated `TryFrom`, which `validate` delegates to.
    assert!(
        tokens.contains("impl :: core :: convert :: TryFrom < Cli > for Greeting"),
        "{tokens}"
    );
    assert!(tokens.contains("name : cli . name"), "{tokens}");
    assert!(tokens.contains("repeat : cli . repeat"), "{tokens}");
    assert!(
        !tokens.contains("Greeting ::"),
        "no conversion is invented: {tokens}"
    );
}

#[test]
fn a_declared_bridge_replaces_the_field_mirror() {
    let tokens = expanded(
        r"
        #[facet(input = Greeting, bridge = example::build)]
        struct Cli {
            name: Option<String>,
        }
        ",
    )
    .expect("a bridge is supported");

    assert!(tokens.contains("example :: build (self)"), "{tokens}");
    assert!(
        !tokens.contains("self . name"),
        "the bridge owns the mapping: {tokens}"
    );
}

#[test]
fn a_struct_without_fields_still_bridges() {
    let tokens = expanded(
        r"
        #[facet(input = Nothing)]
        struct Cli {}
        ",
    )
    .expect("an empty struct has nothing to copy");

    assert!(tokens.contains("let input = Self { }"), "{tokens}");
}

#[test]
fn other_attributes_are_ignored() {
    let tokens = expanded(
        r"
        #[derive(Clone)]
        #[facet(input = Greeting)]
        struct Cli { name: Option<String> }
        ",
    )
    .expect("attributes this derive does not own are clap's business");

    assert!(tokens.contains("type Input = Greeting"), "{tokens}");
}

#[test]
fn a_missing_input_type_is_named_in_the_error() {
    let message = rejected("struct Cli { name: Option<String> }");

    assert!(
        message.contains("#[facet(input = YourInputType)]"),
        "the message must say what to write: {message}"
    );
}

#[test]
fn an_empty_attribute_is_a_missing_input_type() {
    let message = rejected(
        r"
        #[facet()]
        struct Cli { name: Option<String> }
        ",
    );

    assert!(
        message.contains("#[facet(input = YourInputType)]"),
        "{message}"
    );
}

#[test]
fn an_unknown_key_lists_the_supported_ones() {
    let message = rejected(
        r"
        #[facet(input = Greeting, magic = example::item)]
        struct Cli { name: Option<String> }
        ",
    );

    assert!(message.contains("unsupported"), "{message}");
    assert!(
        message.contains("`magic`"),
        "the offending key is named: {message}"
    );
    assert!(
        message.contains("`input`, `bridge`"),
        "the alternatives are listed: {message}"
    );
}

#[test]
fn a_key_that_is_not_an_identifier_is_rejected() {
    let message = rejected(
        r"
        #[facet(input = Greeting, some::path = example::item)]
        struct Cli { name: Option<String> }
        ",
    );

    assert!(message.contains("supported keys"), "{message}");
}

#[test]
fn a_bare_entry_is_rejected() {
    let message = rejected(
        r"
        #[facet(input)]
        struct Cli { name: Option<String> }
        ",
    );

    assert!(message.contains("`key = value` pairs"), "{message}");
}

#[test]
fn a_value_that_is_not_a_path_is_rejected() {
    let message = rejected(
        r"
        #[facet(input = Greeting, bridge = true)]
        struct Cli { name: Option<String> }
        ",
    );

    assert!(message.contains("takes a path"), "{message}");
    assert!(
        message.contains("bridge"),
        "the offending key is named: {message}"
    );
}

#[test]
fn a_duplicated_key_is_rejected() {
    let message = rejected(
        r"
        #[facet(input = Greeting, input = Other)]
        struct Cli { name: Option<String> }
        ",
    );

    assert!(message.contains("declared more than once"), "{message}");
}

#[test]
fn an_enum_is_rejected_as_a_command() {
    let message = rejected(
        r"
        #[facet(input = Greeting)]
        enum Cli { Run }
        ",
    );

    assert!(message.contains("enums and unions"), "{message}");
}

#[test]
fn a_union_is_rejected_as_a_command() {
    let message = rejected(
        r"
        #[facet(input = Greeting)]
        union Cli { value: u8 }
        ",
    );

    assert!(message.contains("enums and unions"), "{message}");
}

#[test]
fn a_tuple_struct_is_rejected_as_a_command() {
    let message = rejected(
        r"
        #[facet(input = Greeting)]
        struct Cli(Option<String>);
        ",
    );

    assert!(message.contains("tuple and unit structs"), "{message}");
}

#[test]
fn a_unit_struct_is_rejected_as_a_command() {
    let message = rejected(
        r"
        #[facet(input = Greeting)]
        struct Cli;
        ",
    );

    assert!(message.contains("tuple and unit structs"), "{message}");
}

// --- `#[clifacet::command(...)]`: clap and the derive, in one attribute -------

/// Expands the attribute on the given source, as the macro would.
fn command(attribute: &str, source: &str) -> syn::Result<String> {
    expand_command(
        &attribute.parse().expect("the attribute parses"),
        source.parse().expect("the item parses"),
    )
    .map(|tokens| tokens.to_string())
}

/// Expands an attribute that must be rejected, returning the reported message.
fn rejected_command(attribute: &str, source: &str) -> String {
    command(attribute, source)
        .err()
        .map(|error| error.to_string())
        .expect("this attribute must be rejected")
}

#[test]
fn the_command_attribute_wraps_clap_and_the_derive_together() {
    let tokens = command(
        "input = Greeting",
        r"
        struct Cli {
            name: Option<String>,
        }
        ",
    )
    .expect("a named struct with an input type is supported");

    assert!(
        tokens.contains(" [derive (:: clap :: Parser , :: clifacet :: Facet)]"),
        "{tokens}"
    );
    assert!(tokens.contains(" [facet (input = Greeting)]"), "{tokens}");
    assert!(
        tokens.contains("struct Cli"),
        "the item is re-emitted: {tokens}"
    );
}

#[test]
fn the_command_attribute_keeps_the_item_untouched() {
    let source = r#"
        #[derive(Debug)]
        #[command(name = "example", about = "A command")]
        struct Cli {
            /// The name to greet.
            #[arg(long, alias = "nickname")]
            name: Option<String>,
        }
        "#;

    let tokens = command("input = Greeting", source).expect("a full command struct is supported");

    for kept in [
        "# [derive (Debug)]",
        "# [command (name = \"example\" , about = \"A command\")]",
        "# [doc = \" The name to greet.\"]",
        "# [arg (long , alias = \"nickname\")]",
    ] {
        assert!(tokens.contains(kept), "`{kept}` was dropped: {tokens}");
    }
}

#[test]
fn the_command_attribute_passes_a_bridge_through() {
    let tokens = command(
        "input = Greeting, bridge = example::build",
        "struct Cli { name: Option<String> }",
    )
    .expect("a bridge is supported");

    assert!(
        tokens.contains(" [facet (input = Greeting , bridge = example :: build)]"),
        "{tokens}"
    );
}

#[test]
fn the_command_attribute_requires_an_input_type_and_says_so_in_its_own_name() {
    let message = rejected_command("", "struct Cli { name: Option<String> }");

    assert!(
        message.contains("[clifacet::command(input = YourInputType)]"),
        "the message must name the attribute the caller wrote: {message}"
    );
}

#[test]
fn the_command_attribute_rejects_an_enum_because_the_bridge_needs_a_struct() {
    let message = rejected_command("input = Greeting", "enum Cli { A, B }");

    assert!(message.contains("struct"), "{message}");
}

#[test]
fn the_command_attribute_refuses_to_derive_claps_parser_twice() {
    let message = rejected_command(
        "input = Greeting",
        r"
        #[derive(clap::Parser)]
        struct Cli { name: Option<String> }
        ",
    );

    assert!(message.contains("Parser"), "{message}");
    assert!(
        message.contains("[clifacet::command"),
        "the message should point at the attribute that already does it: {message}"
    );
}

#[test]
fn the_command_attribute_refuses_to_derive_facet_twice() {
    let message = rejected_command(
        "input = Greeting",
        r"
        #[derive(clifacet::Facet)]
        struct Cli { name: Option<String> }
        ",
    );

    assert!(message.contains("Facet"), "{message}");
}

#[test]
fn the_command_attribute_refuses_a_second_facet_attribute() {
    let message = rejected_command(
        "input = Greeting",
        r"
        #[facet(input = Greeting)]
        struct Cli { name: Option<String> }
        ",
    );

    assert!(
        message.contains("facet"),
        "two `#[facet(...)]` attributes would declare the input twice: {message}"
    );
}

#[test]
fn the_command_attribute_leaves_the_derive_available_on_its_own() {
    // The wrapper is sugar, not a replacement: `#[derive(Facet)]` still works, and the
    // wrapper emits exactly that derive rather than a second implementation of it.
    let wrapped = command("input = Greeting", "struct Cli { name: Option<String> }")
        .expect("the wrapper expands");
    let derived = expanded(
        r"
        #[facet(input = Greeting)]
        struct Cli { name: Option<String> }
        ",
    )
    .expect("the derive expands");

    assert!(wrapped.contains("Facet"), "{wrapped}");
    assert!(
        derived.contains("impl :: clifacet :: Facet for Cli"),
        "{derived}"
    );
}

// --- Field rules: `#[facet(..)]` on a field, and what `#[clifacet(..)]` forwards ---

#[test]
fn field_rules_are_applied_in_one_fixed_order_whatever_order_they_were_written_in() {
    let tokens = expanded(
        r#"
        #[facet(input = Greeting)]
        struct Cli {
            #[arg(long)]
            #[facet(non_empty, trim, fill = "world")]
            name: Option<String>,
        }
        "#,
    )
    .expect("the rules apply to text");

    let fill = tokens.find("__derive :: fill").expect("fill is generated");
    let trim = tokens.find("Text :: trim").expect("trim is generated");
    let non_empty = tokens
        .find("Text :: non_empty")
        .expect("non_empty is generated");
    assert!(
        fill < trim && trim < non_empty,
        "fill, trim, non_empty: {tokens}"
    );
    assert!(
        tokens.contains("flag :: < Cli > (\"name\")"),
        "names the flag lazily: {tokens}"
    );
}

#[test]
fn a_command_without_questions_collects_by_returning_itself() {
    let tokens =
        expanded("#[facet(input = Greeting)] struct Cli { name: Option<String> }").expect("valid");
    assert!(
        tokens.contains("impl :: clifacet :: prompt :: Collect for Cli"),
        "{tokens}"
    );
    assert!(tokens.contains("_session"), "nothing to ask: {tokens}");
    assert!(
        !tokens.contains("CommandFactory"),
        "no command is built: {tokens}"
    );
}

#[test]
fn questions_are_asked_by_the_id_clap_knows_the_field_by() {
    let tokens = expanded(
        r#"
        #[facet(input = Greeting)]
        struct Cli {
            #[arg(long, id = "who")]
            #[facet(ask = "Who?")]
            name: Option<String>,
            #[arg(long)]
            #[facet(confirm = "Sure?")]
            explode: bool,
        }
        "#,
    )
    .expect("valid");
    assert!(
        tokens.contains("ask_for (& mut self . name , & command , \"who\" , \"Who?\")"),
        "{tokens}"
    );
    assert!(
        tokens.contains("confirm_flag (self . explode , & command , \"explode\" , \"Sure?\")"),
        "{tokens}"
    );
}

#[test]
fn a_field_rule_is_checked_for_shape_and_combination() {
    let cases = [
        ("#[facet(trimm)]", "`trimm` is not a field key"),
        ("#[facet(a::b)]", "this key is not a field key"),
        ("#[facet(ask)]", "`ask` takes a value"),
        ("#[facet(trim = true)]", "`trim` takes no value"),
        ("#[facet(trim, trim)]", "`trim` is declared twice"),
        ("#[facet(ask = who)]", "a question is a string literal"),
        ("#[facet(confirm = 1)]", "a question is a string literal"),
        (r#"#[facet(ask = "a", confirm = "b")]"#, "either asked for"),
        (
            r#"#[facet(ask = "a", secret)]"#,
            "would echo the credential",
        ),
    ];
    for (rules, expected) in cases {
        let message = rejected(&format!(
            "#[facet(input = Greeting)] struct Cli {{ #[arg(long)] {rules} name: Option<String> }}"
        ));
        assert!(
            message.contains(expected),
            "{rules}: expected {expected:?} in: {message}"
        );
    }
}

#[test]
fn an_asked_field_has_to_be_a_named_option() {
    let message = rejected(
        r#"#[facet(input = Greeting)] struct Cli { #[arg(value_name = "N")] #[facet(ask = "Who?")] name: Option<String> }"#,
    );
    assert!(message.contains("needs a named option"), "{message}");
}

#[test]
fn a_rule_beside_a_declared_bridge_is_rejected_but_a_question_is_not() {
    let message = rejected(
        "#[facet(input = Greeting, bridge = build)] struct Cli { #[arg(long)] #[facet(fill = 1)] n: Option<u8> }",
    );
    assert!(
        message.contains("`fill` is a rule for the generated bridge"),
        "{message}"
    );

    let tokens = expanded(
        r#"#[facet(input = Greeting, bridge = build)] struct Cli { #[arg(long)] #[facet(ask = "N?")] n: Option<u8> }"#,
    )
    .expect("collection is independent of the bridge");
    assert!(tokens.contains("build (self)"), "{tokens}");
    assert!(
        !tokens.contains("TryFrom"),
        "the declared bridge replaces the generated one: {tokens}"
    );
}

#[test]
fn range_becomes_claps_ranged_parser_for_the_parsed_type() {
    let cases = [
        ("u8", "value_parser ! (u8)"),
        ("Option<u8>", "value_parser ! (u8)"),
        ("Vec<u16>", "value_parser ! (u16)"),
        ("Option<Vec<i64>>", "value_parser ! (i64)"),
        (
            "std::num::NonZeroU8",
            "value_parser ! (std :: num :: NonZeroU8)",
        ),
    ];
    for (ty, parser) in cases {
        let tokens = command(
            "input = Greeting",
            // Spaced: proc_macro2's string lexer reads `1..=8` as the float `1.`; rustc does
            // not, which `tests/ui/field_attribute_valid.rs` proves with the tight spelling.
            &format!("struct Cli {{ #[clifacet(long, range = 1 ..= 8)] n: {ty} }}"),
        )
        .expect("range is supported");
        assert!(tokens.contains(parser), "{ty}: {tokens}");
        assert!(tokens.contains(". range (1 ..= 8)"), "{ty}: {tokens}");
        assert!(
            !tokens.contains("range ="),
            "{ty}: no `range` key reaches clap: {tokens}"
        );
    }
}

#[test]
fn range_beside_a_value_parser_is_two_parsers_for_one_argument() {
    let message = rejected_command(
        "input = Greeting",
        "struct Cli { #[clifacet(long, range = 1 ..= 8, value_parser = parse)] n: u8 }",
    );
    assert!(message.contains("`range` already builds"), "{message}");
}

#[test]
fn grammar_keys_are_accepted_in_the_spelling_claps_derive_accepts() {
    let keys = [
        "required = true",
        "hide = true",
        "env",
        "env = \"APP_N\"",
        "default_value_t",
        "value_parser",
        "action",
        "visible_alias = \"count\"",
    ];
    for key in keys {
        let tokens = command(
            "input = Greeting",
            &format!("struct Cli {{ #[clifacet(long, {key})] n: u8 }}"),
        )
        .unwrap_or_else(|error| panic!("{key}: clap's own spelling is accepted: {error}"));
        assert!(tokens.contains("# [arg (long ,"), "{key}: {tokens}");
    }
}

#[test]
fn grammar_keys_in_a_spelling_claps_derive_rejects_are_reported_here() {
    let cases = [
        ("required", "`required` takes a value"),
        ("value_enum = true", "`value_enum` takes no value"),
        ("long_alias = \"x\"", "`long_alias` is not a"),
    ];
    for (key, expected) in cases {
        let message = rejected_command(
            "input = Greeting",
            &format!("struct Cli {{ #[clifacet(long, {key})] n: u8 }}"),
        );
        assert!(
            message.contains(expected),
            "{key}: expected {expected:?} in: {message}"
        );
    }
}

#[test]
fn interaction_and_normalization_keys_are_forwarded_to_the_derive() {
    let tokens = command(
        "input = Greeting",
        r#"struct Cli { #[clifacet(long, ask = "Who?", trim)] name: Option<String> }"#,
    )
    .expect("forwarded");
    assert!(
        tokens.contains("# [arg (long)]"),
        "grammar to clap: {tokens}"
    );
    assert!(
        tokens.contains("# [facet (ask = \"Who?\" , trim)]"),
        "rules to the derive: {tokens}"
    );
}

#[test]
fn a_type_that_is_not_an_option_or_a_vec_of_one_type_is_parsed_as_itself() {
    let cases = [
        // Not a path at all.
        ("[u8; 2]", "value_parser ! ([u8 ; 2])"),
        // A path, but not a container this peels.
        (
            "Result<u8, String>",
            "value_parser ! (Result < u8 , String >)",
        ),
        // `Option` without angle brackets is a path with no type to peel.
        ("Option", "value_parser ! (Option)"),
        // `Vec` with two type arguments (an allocator) is not `Vec<T>`.
        ("Vec<u8, Global>", "value_parser ! (Vec < u8 , Global >)"),
        // A lifetime argument is not a type.
        ("Option<'static>", "value_parser ! (Option < 'static >)"),
    ];
    for (ty, parser) in cases {
        let tokens = command(
            "input = Greeting",
            &format!("struct Cli {{ #[clifacet(long, range = 1 ..= 8)] n: {ty} }}"),
        )
        .expect("range is forwarded whatever the type; clap reports a type it cannot range");
        assert!(tokens.contains(parser), "{ty}: {tokens}");
    }
}

#[test]
fn an_id_that_is_not_a_string_literal_leaves_the_field_name_as_the_id() {
    for id in ["id = 5", "id = NAME"] {
        let tokens = expanded(&format!(
            r#"
            #[facet(input = Greeting)]
            struct Cli {{
                #[arg(long, {id})]
                #[facet(ask = "Who?")]
                name: Option<String>,
            }}
            "#
        ))
        .expect("the derive leaves a malformed id to clap");
        assert!(
            tokens.contains("& command , \"name\" , \"Who?\""),
            "{id}: {tokens}"
        );
    }
}

// --- `input = Type`: any type the generated impls can name (CLI-35) ----------

#[test]
fn a_generic_input_type_is_named_whole_in_every_generated_impl() {
    let tokens = expanded(
        r"
        #[facet(input = Greeting<String>)]
        struct Cli { name: Option<String> }
        ",
    )
    .expect("a generic input type is a type, not an expression");

    assert!(
        tokens.contains("type Input = Greeting < String >"),
        "{tokens}"
    );
    assert!(
        tokens.contains("TryFrom < Cli > for Greeting < String >"),
        "{tokens}"
    );
}

#[test]
fn a_qualified_self_type_keeps_its_qualification() {
    let tokens = expanded(
        r"
        #[facet(input = <Marker as Shape>::Input)]
        struct Cli { name: Option<String> }
        ",
    )
    .expect("a projection names one type");

    assert!(
        tokens.contains("type Input = < Marker as Shape > :: Input"),
        "the `<Marker as Shape>` part must survive: {tokens}"
    );
}

#[test]
fn the_generated_bridge_builds_the_input_through_self_whatever_its_spelling() {
    let tokens = expanded(
        r"
        #[facet(input = crate::inputs::Greeting<'static>)]
        struct Cli { name: Option<String> }
        ",
    )
    .expect("a qualified generic path is supported");

    // `Greeting<'static> { .. }` is not an expression; `Self { .. }` is, for every type.
    assert!(
        tokens.contains("let input = Self { name : cli . name }"),
        "{tokens}"
    );
}

#[test]
fn the_command_structs_generics_reach_every_generated_impl() {
    let tokens = expanded(
        r"
        #[facet(input = Greeting<'a>)]
        struct Cli<'a> { name: Option<String>, #[facet(skip)] marker: PhantomData<&'a ()> }
        ",
    )
    .expect("a lifetime the command struct declares can appear in the input");

    for generated in [
        "impl < 'a > :: clifacet :: Facet for Cli < 'a >",
        "impl < 'a > :: core :: convert :: TryFrom < Cli < 'a > > for Greeting < 'a >",
        "impl < 'a > :: clifacet :: prompt :: Collect for Cli < 'a >",
    ] {
        assert!(
            tokens.contains(generated),
            "missing `{generated}`: {tokens}"
        );
    }
}

#[test]
fn a_bridge_and_a_check_keep_their_qualification() {
    let bridged = expanded(
        r"
        #[facet(input = Greeting, bridge = <Greeting as Build>::build)]
        struct Cli { name: Option<String> }
        ",
    )
    .expect("a qualified bridge path is supported");
    let checked = expanded(
        r"
        #[facet(input = Greeting, check = <Greeting as Rule>::check)]
        struct Cli { name: Option<String> }
        ",
    )
    .expect("a qualified check path is supported");

    assert!(
        bridged.contains("< Greeting as Build > :: build (self)"),
        "{bridged}"
    );
    assert!(
        checked.contains("< Greeting as Rule > :: check"),
        "{checked}"
    );
}

#[test]
fn an_input_that_is_not_a_type_says_what_input_takes() {
    let message = rejected(
        r"
        #[facet(input = 1 + 2)]
        struct Cli { name: Option<String> }
        ",
    );

    assert_eq!(
        message,
        "`input` takes a type, for example `input = my_module::MyInput`"
    );
}

#[test]
fn self_in_the_input_type_is_rejected_because_it_names_the_command() {
    let message = rejected(
        r"
        #[facet(input = Self::Input)]
        struct Cli { name: Option<String> }
        ",
    );

    assert_eq!(
        message,
        "`input` cannot mention `Self`: the generated impls are for the command struct, \
         so `Self` would be the command or the input itself; name the type directly"
    );
}

#[test]
fn a_lifetime_the_command_struct_does_not_declare_is_rejected() {
    let message = rejected(
        r"
        #[facet(input = Greeting<'a>)]
        struct Cli { name: Option<String> }
        ",
    );

    assert_eq!(
        message,
        "`'a` is not declared on the command struct, so the generated impls cannot name it: \
         declare it there (`struct Cli<'a>`) or use `'static`"
    );
}

#[test]
fn static_and_higher_ranked_lifetimes_need_no_declaration() {
    expanded(
        r"
        #[facet(input = Greeting<'static>, bridge = build)]
        struct Cli { name: Option<String> }
        ",
    )
    .expect("`'static` is always in scope");
    expanded(
        r"
        #[facet(input = Box<dyn for<'b> Fn(&'b str) -> String>, bridge = build)]
        struct Cli { name: Option<String> }
        ",
    )
    .expect("a lifetime bound by `for<..>` is declared where it is used");
}

#[test]
fn an_elided_lifetime_is_rejected() {
    let message = rejected(
        r"
        #[facet(input = Greeting<'_>)]
        struct Cli { name: Option<String> }
        ",
    );

    assert_eq!(
        message,
        "`input` needs every lifetime named: `'_` cannot appear in an associated type; \
         declare a lifetime on the command struct or use `'static`"
    );
}

#[test]
fn an_inferred_or_opaque_input_type_is_rejected() {
    for (source, written) in [("_", "_"), ("impl Iterator<Item = u8>", "impl Trait")] {
        let message = rejected(&format!(
            "#[facet(input = {source}, bridge = build)] struct Cli {{ name: Option<String> }}"
        ));

        assert_eq!(
            message,
            format!(
                "`input` names the one type `validate` returns, so `{written}` cannot stand \
                 for it: write the concrete type"
            )
        );
    }
}

#[test]
fn an_input_that_is_not_a_struct_path_needs_a_declared_bridge() {
    let message = rejected(
        r"
        #[facet(input = (String, u8))]
        struct Cli { name: Option<String> }
        ",
    );
    expanded(
        r"
        #[facet(input = (String, u8), bridge = build)]
        struct Cli { name: Option<String> }
        ",
    )
    .expect("a declared bridge can build any type");

    assert_eq!(
        message,
        "without `bridge = path`, the generated bridge builds the input field by field, so \
         `input` has to name a struct; declare `bridge = path` to build this type yourself"
    );
}

#[test]
fn the_command_attribute_takes_the_same_input_types() {
    let tokens = command(
        "input = <Marker as Shape>::Input",
        "struct Cli { name: Option<String> }",
    )
    .expect("the attribute reads `input` as a type too");

    assert!(
        tokens.contains(" [facet (input = < Marker as Shape >:: Input)]"),
        "{tokens}"
    );
    assert!(
        command(
            "input = Greeting<String>",
            "struct Cli { name: Option<String> }"
        )
        .is_ok(),
        "a generic input type is not an expression"
    );
}

#[test]
fn every_offending_token_in_the_input_type_is_reported_at_once() {
    let source = r"
        #[facet(input = Pair<Self, 'a>)]
        struct Cli { name: Option<String> }
        ";
    let input: DeriveInput = syn::parse_str(source).expect("the test source parses");
    let error = expand(&input).expect_err("both tokens are rejected");

    let messages: Vec<String> = error.into_iter().map(|error| error.to_string()).collect();

    assert_eq!(messages.len(), 2, "{messages:?}");
    assert!(messages[0].contains("`Self`"), "{messages:?}");
    assert!(messages[1].contains("`'a` is not declared"), "{messages:?}");
}

#[test]
fn a_bare_function_binds_its_own_lifetimes_and_type_parameters_too() {
    for input in [
        "for<'b> fn(&'b str) -> String",
        // syn reads a type parameter in a binder; it binds no lifetime, so `'b` still must.
        "Box<dyn for<'b, T> Fn(&'b T)>",
    ] {
        expanded(&format!(
            "#[facet(input = {input}, bridge = build)] struct Cli {{ name: Option<String> }}"
        ))
        .expect("a lifetime bound by `for<..>` is in scope where it is used");
    }
}

#[test]
fn a_parenthesized_or_invisibly_grouped_struct_path_still_names_a_struct() {
    expanded(
        r"
        #[facet(input = (Greeting))]
        struct Cli { name: Option<String> }
        ",
    )
    .expect("parentheses around a path change nothing");

    // A `macro_rules!` that forwards `$input:ty` wraps it in an invisible group.
    let grouped = syn::Type::Group(syn::TypeGroup {
        group_token: syn::token::Group::default(),
        elem: Box::new(syn::parse_quote!(Greeting)),
    });
    crate::input_type::check(&grouped, &syn::Generics::default(), false)
        .expect("an invisible group around a path changes nothing");
}

// --- command trees (CLI-32) -------------------------------------------------------

/// Expands `#[clifacet::subcommands(attribute)]` on the given enum, as the macro would.
fn tree(attribute: &str, source: &str) -> syn::Result<String> {
    let attribute: proc_macro2::TokenStream = attribute.parse().expect("the attribute lexes");
    let item: proc_macro2::TokenStream = source.parse().expect("the item lexes");
    crate::subcommands::expand(attribute, item).map(|tokens| tokens.to_string())
}

fn rejected_tree(attribute: &str, source: &str) -> String {
    tree(attribute, source)
        .err()
        .map(|error| error.to_string())
        .expect("this tree must be rejected")
}

const TWO_LEAVES: &str = "enum Commands { Add(Add), Done(Done) }";

#[test]
fn a_tree_is_claps_subcommand_with_its_conversion_and_collection() {
    let tokens = tree("input = Request", TWO_LEAVES).expect("a tree of leaves");

    assert!(
        tokens.starts_with("# [derive (:: clap :: Subcommand)]"),
        "{tokens}"
    );
    assert!(
        tokens.contains("impl :: core :: convert :: TryFrom < Commands > for Request"),
        "{tokens}"
    );
    assert!(
        tokens.contains(
            "Commands :: Add (leaf) => Self :: Add (< Add as :: clifacet :: Facet > :: validate (leaf) ?)"
        ),
        "{tokens}"
    );
    // Naming every variant once more is what makes an extra `Request` variant an error.
    assert!(
        tokens.contains(
            "| request : & Self | match request { Self :: Add (_) => { } Self :: Done (_) => { } }"
        ),
        "{tokens}"
    );
    assert!(
        tokens.contains("impl :: clifacet :: facet :: __derive :: Tree for Commands"),
        "{tokens}"
    );
    assert!(
        tokens.contains("impl :: clifacet :: prompt :: Collect for Commands"),
        "{tokens}"
    );
    assert!(
        !tokens.contains("Command for Commands"),
        "no output, no dispatch: {tokens}"
    );
    assert!(!tokens.contains("Planned for Commands"), "{tokens}");
}

#[test]
fn a_tree_with_an_output_runs_the_chosen_leaf() {
    let tokens = tree("input = Request, output = Report", TWO_LEAVES).expect("a running tree");

    assert!(
        tokens.contains("impl :: clifacet :: Command for Commands"),
        "{tokens}"
    );
    assert!(tokens.contains("type Output = Report"), "{tokens}");
    assert!(
        tokens.contains("type __ClifacetRequest = Request"),
        "{tokens}"
    );
    assert!(
        tokens.contains("(Self :: Add (leaf) , __ClifacetRequest :: Add (input)) => < Add as :: clifacet :: Command > :: execute (leaf , input) . map (:: core :: convert :: Into :: into)"),
        "{tokens}"
    );
    assert!(
        tokens.contains("Failure :: execution"),
        "a mismatched pair fails: {tokens}"
    );
    assert!(!tokens.contains("Planned for Commands"), "{tokens}");
}

#[test]
fn a_planned_tree_hands_the_chosen_leafs_plan_up_instead() {
    let tokens = tree("input = Request, output = Report, planned", TWO_LEAVES).expect("planned");

    assert!(
        tokens.contains("impl :: clifacet :: command :: Planned for Commands"),
        "{tokens}"
    );
    assert!(
        tokens.contains("< Add as :: clifacet :: command :: Planned > :: plan (leaf , input) . map (| plan | plan . map (:: core :: convert :: Into :: into))"),
        "{tokens}"
    );
    // `Command` comes from the library's blanket impl for every `Planned`.
    assert!(
        !tokens.contains("impl :: clifacet :: Command for Commands"),
        "{tokens}"
    );
}

#[test]
fn variant_keys_become_claps_command_attributes_and_raw_ones_pass_through() {
    let tokens = tree(
        "input = Request",
        r#"enum Commands {
            #[clifacet(name = "add-task", alias = "new", visible_alias = "a", hide)]
            #[command(long_about = "Adds one")]
            Add(Add),
        }"#,
    )
    .expect("known variant keys");

    assert!(
        tokens.contains(r#"# [command (name = "add-task")]"#),
        "{tokens}"
    );
    assert!(
        tokens.contains(r#"# [command (alias = "new")]"#),
        "{tokens}"
    );
    assert!(
        tokens.contains(r#"# [command (visible_alias = "a")]"#),
        "{tokens}"
    );
    assert!(tokens.contains("# [command (hide = true)]"), "{tokens}");
    assert!(
        tokens.contains(r#"# [command (long_about = "Adds one")]"#),
        "{tokens}"
    );
    assert!(!tokens.contains("clifacet ("), "consumed: {tokens}");
}

#[test]
fn a_tree_is_an_enum_without_generics_with_at_least_one_leaf() {
    assert!(
        rejected_tree("input = Request", "struct Commands { add: Add }")
            .contains("applies to an enum, one variant per subcommand"),
    );
    assert!(
        rejected_tree("input = Request", "enum Commands<T> { Add(T) }")
            .starts_with("a command tree takes no generics"),
    );
    assert!(
        rejected_tree("input = Request", "enum Commands {}")
            .starts_with("a command tree needs at least one subcommand"),
    );
    assert!(
        rejected_tree(
            "input = Request",
            "#[derive(Debug, clap::Subcommand)] enum Commands { Add(Add) }"
        )
        .starts_with("`Subcommand` is derived twice"),
    );
}

#[test]
fn a_variant_is_one_leaf_struct_and_the_error_says_how_to_write_it() {
    let inline = rejected_tree("input = Request", "enum Commands { Add { title: String } }");
    assert!(
        inline.starts_with("`Add` declares its arguments inline"),
        "{inline}"
    );
    assert!(
        inline.contains("write a leaf struct instead: `Add(Add)`, with `#[clifacet::command(input = AddInput)] struct Add { … }`"),
        "{inline}"
    );

    let unit = rejected_tree("input = Request", "enum Commands { Doctor }");
    assert!(unit.starts_with("`Doctor` has no leaf"), "{unit}");
    assert!(
        unit.contains("write a leaf struct instead: `Doctor(Doctor)`"),
        "{unit}"
    );
    assert!(unit.ends_with("which may be empty"), "{unit}");

    let pair = rejected_tree("input = Request", "enum Commands { Add(Add, Done) }");
    assert!(
        pair.starts_with("`Add` holds more than one value"),
        "{pair}"
    );
}

#[test]
fn the_tree_attribute_takes_known_keys_once_each() {
    assert_eq!(
        rejected_tree("input = Request, dispatch = auto", TWO_LEAVES),
        "`dispatch` is not a key of `#[clifacet::subcommands(..)]`; supported keys: \
         `input = Type`, `output = Type`, `planned`, `context = Type`"
    );
    assert_eq!(
        rejected_tree("input = Request, input = Other", TWO_LEAVES),
        "`input` is declared twice; one key, one meaning"
    );
    assert_eq!(
        rejected_tree("input", TWO_LEAVES),
        "`input` takes a type, for example `input = MyType`"
    );
    assert_eq!(
        rejected_tree(
            "input = Request, output = Report, planned = yes",
            TWO_LEAVES
        ),
        "`planned` takes no value: write `planned` on its own"
    );
    assert!(
        rejected_tree("input = Request, planned", TWO_LEAVES).starts_with(
            "`planned` makes the tree a planned command, and a command needs its output"
        ),
    );
    assert!(rejected_tree("output = Report", TWO_LEAVES).contains(
        "needs the request it converts to: add `#[clifacet::subcommands(input = YourRequest)]`"
    ),);
}

#[test]
fn a_tree_with_a_context_is_a_handler_of_it_and_reaches_each_leaf_as_one() {
    let tokens = tree(
        "input = Request, output = Report, context = Store",
        TWO_LEAVES,
    )
    .expect("a tree with a context");
    assert!(
        tokens.contains("impl :: clifacet :: Handler < Store > for Commands"),
        "{tokens}"
    );
    assert!(
        tokens.contains("< Add as :: clifacet :: Handler < Store >> :: handle"),
        "{tokens}"
    );
    assert!(
        tokens.contains("< Done as :: clifacet :: Handler < Store >> :: preview"),
        "{tokens}"
    );
    // A tree with a context is a handler, not a command: a context has to be supplied.
    assert!(
        !tokens.contains("impl :: clifacet :: Command for"),
        "{tokens}"
    );
}

#[test]
fn a_context_needs_an_output_and_makes_planned_redundant() {
    assert!(
        rejected_tree("input = Request, context = Store", TWO_LEAVES)
            .starts_with("`context` hands the context to the leaf that runs, so the tree runs"),
    );
    assert!(
        rejected_tree(
            "input = Request, output = Report, planned, context = Store",
            TWO_LEAVES
        )
        .ends_with("so `planned` has nothing to add: drop it"),
    );
    assert_eq!(
        rejected_tree(
            "input = Request, output = Report, context = Store, context = Other",
            TWO_LEAVES
        ),
        "`context` is declared twice; one key, one meaning"
    );
    assert_eq!(
        rejected_tree("input = Request, output = Report, context", TWO_LEAVES),
        "`context` takes a type, for example `context = MyType`"
    );
}

#[test]
fn a_variant_takes_known_keys_once_each_in_their_own_shape() {
    let variant = |attribute: &str| {
        rejected_tree(
            "input = Request",
            &format!("enum Commands {{ #[clifacet({attribute})] Add(Add) }}"),
        )
    };

    let unknown = variant(r#"about = "Adds""#);
    assert!(
        unknown.starts_with("`about` is not a variant key of `#[clifacet(..)]`; supported: `alias = \"…\"`, `name = \"…\"`, `visible_alias = \"…\"`, `hide`"),
        "{unknown}"
    );
    assert!(
        unknown.ends_with("raw `#[command(..)]` on the variant"),
        "{unknown}"
    );
    assert!(variant("a::b").starts_with("this key is not a variant key"));
    assert_eq!(
        variant(r#"alias = "x", alias = "y""#),
        "`alias` is declared twice on this variant; one key, one meaning"
    );
    assert_eq!(
        variant("name"),
        "`name` takes a string, for example `name = \"…\"`"
    );
    assert_eq!(
        variant("name = 3"),
        "`name` takes a string, for example `name = \"…\"`"
    );
    assert_eq!(
        variant("hide = true"),
        "`hide` takes no value: write `hide` on its own"
    );
}

#[test]
fn a_name_two_subcommands_answer_to_is_reported_at_the_second() {
    assert_eq!(
        rejected_tree(
            "input = Request",
            r#"enum Commands { Add(Add), #[clifacet(alias = "add")] Done(Done) }"#
        ),
        "`alias = \"add\"` answers to `add`, and so does `Add`: one name, one subcommand — \
         rename one of them, or drop the alias"
    );
    // A default name is clap's kebab-case of the variant, and it is described as one.
    assert_eq!(
        rejected_tree(
            "input = Request",
            r#"enum Commands { #[clifacet(alias = "list-all")] Show(Show), ListAll(ListAll) }"#
        ),
        "`ListAll`, named `list-all` by default, answers to `list-all`, and so does `Show`: \
         one name, one subcommand — rename one of them, or drop the alias"
    );
    // Raw clap names count too.
    assert!(
        rejected_tree(
            "input = Request",
            r#"enum Commands { Add(Add), #[command(name = "add")] Done(Done) }"#
        )
        .starts_with("`name = \"add\"` answers to `add`, and so does `Add`"),
    );
}

#[test]
fn an_explicit_name_replaces_the_default_and_a_renaming_enum_is_left_to_clap() {
    tree(
        "input = Request",
        r#"enum Commands { #[clifacet(name = "plus")] Add(Add), #[clifacet(alias = "add")] Done(Done) }"#,
    )
    .expect("`add` is no longer `Add`'s name");
    tree(
        "input = Request",
        r#"#[command(rename_all = "lower")] enum Commands { ListAll(ListAll), #[clifacet(alias = "list-all")] Show(Show) }"#,
    )
    .expect("clap computes the names when the enum renames them");
}

#[test]
fn a_subcommands_field_converts_and_collects_through_the_tree() {
    let required = expanded(
        r"
        #[facet(input = Input)]
        struct Cli {
            #[facet(subcommands)]
            command: Commands,
        }
        ",
    )
    .expect("a required tree");
    assert!(
        required.contains("command : :: clifacet :: facet :: __derive :: tree (cli . command) ?"),
        "{required}"
    );
    assert!(
        required.contains(
            "self . command = :: clifacet :: facet :: __derive :: collect_tree (self . command , session) ?"
        ),
        "{required}"
    );
    assert!(
        !required.contains("CommandFactory"),
        "nothing is asked by flag, so clap's command is not built: {required}"
    );

    let optional = expanded(
        r"
        #[facet(input = Input)]
        struct Cli {
            #[facet(subcommands)]
            command: Option<Commands>,
        }
        ",
    )
    .expect("an optional tree");
    assert!(
        optional.contains(
            "command : :: clifacet :: facet :: __derive :: optional_tree (cli . command) ?"
        ),
        "{optional}"
    );
    assert!(
        optional.contains("collect_optional_tree (self . command , session ,) ?"),
        "{optional}"
    );
}

#[test]
fn a_subcommands_field_stands_alone() {
    assert_eq!(
        rejected(
            r"
            #[facet(input = Input)]
            struct Cli { #[facet(subcommands, skip)] command: Commands }
            ",
        ),
        "`skip` beside `subcommands` would apply to the whole tree, and a tree is not one \
         value: declare the rule on the leaf struct's own field instead"
    );
    assert!(
        rejected(
            r"
            #[facet(input = Input, bridge = build)]
            struct Cli { #[facet(subcommands)] command: Commands }
            ",
        )
        .starts_with("`subcommands` is a rule for the generated bridge"),
    );
    assert!(
        rejected_command(
            "input = Input",
            "struct Cli { #[clifacet(subcommands, long)] command: Commands }"
        )
        .starts_with("a `subcommands` field is a command tree, not an argument"),
    );
}

#[test]
fn the_command_attribute_marks_a_subcommands_field_for_clap() {
    let tokens = command(
        "input = Input",
        "struct Cli { #[clifacet(subcommands)] command: Option<Commands> }",
    )
    .expect("a tree field");

    assert!(
        tokens.contains(
            "# [command (subcommand)] # [facet (subcommands)] command : Option < Commands >"
        ),
        "{tokens}"
    );
}

#[test]
fn a_declared_collect_replaces_the_generated_questions() {
    let tokens = expanded(
        r"
        #[facet(input = Input, collect = ask_by_hand)]
        struct Cli { #[facet(fill = 1)] count: Option<u8> }
        ",
    )
    .expect("collection by hand");

    assert!(
        tokens.contains("let collect : fn (Self , & mut :: clifacet :: prompt :: Session ,) -> :: core :: result :: Result < Self , :: clifacet :: Failure > = ask_by_hand ; collect (self , session)"),
        "{tokens}"
    );
    assert!(!tokens.contains("ask_for"), "{tokens}");
}

#[test]
fn a_question_beside_a_declared_collect_is_never_asked_so_it_is_rejected() {
    for (rule, key) in [
        ("ask = \"Who?\"", "ask"),
        ("confirm = \"Sure?\"", "confirm"),
    ] {
        let message = rejected(&format!(
            "#[facet(input = Input, collect = by_hand)] struct Cli {{ #[arg(long)] #[facet({rule})] name: Option<String> }}"
        ));
        assert!(
            message.starts_with(&format!(
                "`{key}` is a question for the generated collection"
            )),
            "{message}"
        );
    }
    assert_eq!(
        rejected("#[facet(input = Input, collect = a, collect = b)] struct Cli {}"),
        "`collect` is declared more than once"
    );
}

fn commands(attribute: &str) -> String {
    let tokens: proc_macro2::TokenStream = attribute.parse().expect("the call lexes");
    // `call_site()` rather than a parsed span: every call here names its modules, so
    // discovery — the only reader of the call site — never runs.
    crate::commands::expand(tokens, proc_macro2::Span::call_site()).to_string()
}

fn rejected_commands(attribute: &str) -> String {
    let tokens: proc_macro2::TokenStream = attribute.parse().expect("the call lexes");
    let expanded = crate::commands::expand(tokens, proc_macro2::Span::call_site()).to_string();
    assert!(
        expanded.contains("compile_error"),
        "not rejected: {expanded}"
    );
    expanded
}

#[test]
fn a_commands_call_registers_modules_a_tree_a_request_and_a_root() {
    let tokens = commands("search_history, show, output = Report, planned");

    assert!(tokens.contains("mod search_history ;"), "{tokens}");
    // The struct is the stem in Pascal case, which clap reads as `search-history`.
    // The re-export is `pub(crate)` so the same call works in an application and in a
    // test; the leaf itself may be `pub`, and `pub(crate)` re-exporting it is allowed.
    assert!(
        tokens.contains("pub (crate) use search_history :: SearchHistory ;"),
        "{tokens}"
    );
    // The tree holds the leaves by name, re-exported just above it.
    assert!(
        tokens.contains("pub enum Commands { SearchHistory (SearchHistory)")
            && tokens.contains("Show (Show)"),
        "{tokens}"
    );
    assert!(tokens.contains("pub enum Request {"), "{tokens}");
    // The root: the tree, the globals every application gets, and a name from Cargo.
    assert!(tokens.contains("pub struct Cli"), "{tokens}");
    assert!(
        tokens.contains("pub command : Option < Commands >"),
        "{tokens}"
    );
    assert!(
        tokens.contains("pub globals : :: clifacet :: Globals"),
        "{tokens}"
    );
    assert!(tokens.contains("CARGO_PKG_NAME"), "{tokens}");
}

#[test]
fn a_commands_call_forwards_its_keys_to_the_tree() {
    let tokens = commands("greet, output = Report, context = Store");
    assert!(
        tokens.contains("clifacet :: Handler < Store > for Commands"),
        "{tokens}"
    );
    // With a context the root is a handler of it too, so the runner can run it.
    assert!(
        tokens.contains("impl :: clifacet :: Handler < Store > for Cli"),
        "{tokens}"
    );

    let planned = commands("greet, output = Report, planned");
    assert!(
        planned.contains("impl :: clifacet :: command :: Planned for Commands"),
        "{planned}"
    );
    assert!(
        planned.contains("impl :: clifacet :: command :: Planned for Cli"),
        "{planned}"
    );

    // Without a context or `planned`, the tree is a plain command and so is the root.
    let run = commands("greet, output = Report");
    assert!(
        run.contains("impl :: clifacet :: Command for Commands"),
        "{run}"
    );
    assert!(run.contains("impl :: clifacet :: Command for Cli"), "{run}");
}

#[test]
fn a_commands_call_names_the_request_and_the_roots_own_fields() {
    let tokens =
        commands("list, request = Action, root = { \"json\": bool, \"verbose\": Option<String> }");
    // The tree converts into the name the application gave, and the root validates into
    // an input that carries the root's own fields beside the chosen command.
    assert!(tokens.contains("pub enum Action {"), "{tokens}");
    assert!(
        tokens.contains("TryFrom < Commands > for Action"),
        "{tokens}"
    );
    assert!(tokens.contains("pub json : bool"), "{tokens}");
    assert!(
        tokens.contains("pub verbose : Option < String >"),
        "{tokens}"
    );
    // Each is a global long flag, so it may be written before or after the subcommand.
    assert!(tokens.contains("# [clifacet (long)]"), "{tokens}");
    assert!(tokens.contains("# [arg (global = true)]"), "{tokens}");
    assert!(
        tokens.contains("env ! (\"CARGO_PKG_DESCRIPTION\")"),
        "{tokens}"
    );
}

#[test]
fn a_commands_call_takes_the_root_metadata_it_is_given() {
    let tokens = commands("greet, name = \"bl\", about = \"A wrapper\", version_off");
    assert!(tokens.contains("name = \"bl\""), "{tokens}");
    assert!(tokens.contains("about = \"A wrapper\""), "{tokens}");
    let long = commands("greet, long_about = \"A longer story\"");
    assert!(long.contains("long_about = \"A longer story\""), "{long}");
    assert!(!tokens.contains("version ,"), "no --version: {tokens}");
    assert!(
        commands("greet").contains("version ,"),
        "version by default"
    );
}

#[test]
fn a_commands_call_says_what_it_cannot_register() {
    assert!(
        rejected_commands("list, list")
            .contains("`list` is listed twice; one module, one command — drop the second")
    );
    // A key before any module is still a key: it is reported as one, not as a command.
    assert!(rejected_commands("discover = auto").contains("`discover` is not a key"));
    // A call with no modules and no report is discovery that cannot run: the message says
    // what to add, because the caller wrote keys and forgot the commands (CLI-50).
    assert!(
        rejected_commands("name = \"app\"")
            .contains("reads the report its folder's `build.rs` wrote")
    );
    let unknown = rejected_commands("list, discover = auto");
    assert!(
        unknown.contains("`discover` is not a key of `clifacet::commands!(..)`"),
        "{unknown}"
    );
    assert!(
        unknown.contains("`globals = Type`") && unknown.contains("`version_off`"),
        "{unknown}"
    );
    assert!(
        rejected_commands("list, planned, context = Store")
            .contains("`context` and `planned` cannot both be declared")
    );
    assert!(
        rejected_commands("list, planned = yes")
            .contains("`planned` takes no value: write `planned` on its own")
    );
    assert!(
        rejected_commands("list, output")
            .contains("`output` takes a value, for example `output = MyType`")
    );
    assert!(
        rejected_commands("list, root = [\"json\": bool]")
            .contains("`root` takes the root's own input fields in braces")
    );
}

/// The fixture every registry test reads: a real folder, and the report for it.
///
/// A real folder rather than a temporary one, because a registry includes a file and points
/// its `mod` lines at the files beside it — so what is under test is a folder shaped like an
/// application's. `examples/discovered` is exactly that, and the report is what
/// `clifacet::build::commands()` writes for it: two commands, one group, one helper.
fn fixture() -> (String, String) {
    // Walked up rather than split on a separator: the manifest path uses `\` on Windows, so a
    // `/crates/` split would find nothing there and these tests would panic on that platform.
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the macros crate lives two levels under the workspace root")
        .join("examples")
        .join("discovered")
        .join("src")
        .join("commands")
        .display()
        .to_string();
    let report = "clifacet-commands/1;folder=src/commands;command=echo;\
                  command=list;group=db;helper=_shared"
        .to_owned();
    (folder, report)
}

/// Expands `#[clifacet::registry(..)] mod commands {}` against the fixture folder.
fn registry(attribute: &str) -> String {
    let (folder, report) = fixture();
    expand_registry(attribute, "pub mod commands {}", &folder, &report)
}

/// The same, returning the message instead when the attribute is refused.
fn rejected_registry(attribute: &str) -> String {
    let (folder, report) = fixture();
    expand_registry(attribute, "pub mod commands {}", &folder, &report)
}

/// One expansion, with the report passed rather than read from `CLIFACET_COMMANDS`.
///
/// Separators are normalized to `/` because the paths in an expansion are the platform's: the
/// same assertion would otherwise have to be written twice, once with `\` and once with `/`,
/// and a test that differs by platform proves less than one that does not. The raw string
/// holds *two* backslashes because a path printed as tokens is escaped — Windows' `D:\a` is
/// `D:\\a` in the token stream a test reads.
fn expand_registry(attribute: &str, item: &str, folder: &str, report: &str) -> String {
    crate::commands::registry_for_test(
        &attribute.parse().expect("the attribute lexes"),
        &item.parse().expect("the item lexes"),
        folder,
        report,
    )
    .map_or_else(|error| error.to_string(), |tokens| tokens.to_string())
    .replace(r"\\", "/")
}

#[test]
fn a_registry_pulls_the_folder_in_and_registers_what_the_report_lists() {
    let tokens = registry("");

    // The folder's own file is included, so its `mod` lines and its items are the compiler's
    // to read — the one thing a `commands!` call cannot do.
    assert!(
        tokens.contains("include ! (\""),
        "the folder is included: {tokens}"
    );
    assert!(tokens.contains("/commands/mod.rs\")"), "{tokens}");
    // The leaves are declared here, each with the file it means, because a `mod` a macro emits
    // resolves beside the attribute rather than beside the folder.
    // Every leaf carries the path of its file, which is what the include puts them beside.
    assert!(tokens.contains("# [path = \""), "{tokens}");
    for leaf in ["echo", "list"] {
        assert!(
            tokens.contains(&format!("/{leaf}.rs\"] mod {leaf} ;")),
            "{leaf} is declared at its own file: {tokens}"
        );
        assert!(
            tokens.contains(&format!("pub (crate) use {leaf} :: ")),
            "{leaf} is re-exported: {tokens}"
        );
    }
    // A group is a folder, so it is declared like a leaf, at its own `mod.rs`.
    assert!(tokens.contains("/db/mod.rs\"] mod db ;"), "{tokens}");
    // The registration is the same one `commands!(..)` writes.
    assert!(tokens.contains("pub enum Commands {"), "{tokens}");
    assert!(tokens.contains("pub enum Action {"), "{tokens}");
    assert!(tokens.contains("pub struct Input"), "{tokens}");
    assert!(tokens.contains("pub struct Cli"), "{tokens}");
}

#[test]
fn a_registry_declares_a_helper_and_never_registers_or_reexports_it() {
    let tokens = registry("");
    // Declared, so the folder's own commands can reach it...
    assert!(
        tokens.contains("/_shared.rs\"] # [allow (unused_imports)] mod _shared ;"),
        "the helper is declared at its file: {tokens}"
    );
    // ...and with no re-export: a helper is the folder's business (CLI-50, AC#4), which is
    // why reaching one from outside is E0603 rather than a path that quietly works.
    assert!(
        !tokens.contains("use _shared ::"),
        "a helper is not re-exported: {tokens}"
    );
    // It is not a variant, and it has no input: a helper is not a command.
    assert!(!tokens.contains("_shared (_shared"), "{tokens}");
    assert!(!tokens.contains("_shared ::"), "{tokens}");
}

#[test]
fn an_empty_inline_module_is_filled_from_the_folders_file() {
    // `mod commands {}` is how a folder is declared: outlined modules are unstable in an
    // attribute's input below Rust 1.99, and the crate's floor is 1.88 (decision-21). The
    // braces are what make the module inline; the folder's file is still what fills it.
    let tokens = registry("");
    assert!(tokens.contains("mod commands {"), "{tokens}");
    assert!(tokens.contains("include ! ("), "{tokens}");
    assert!(tokens.contains("/commands/mod.rs\")"), "{tokens}");

    // A body written in place is kept as written and *is* the folder, so nothing is
    // included: the items are already where Rust would have put them.
    let (folder, report) = fixture();
    let inline = expand_registry(
        "",
        "pub mod commands { pub use crate::Said; }",
        &folder,
        &report,
    );
    assert!(inline.contains("pub use crate :: Said ;"), "{inline}");
    assert!(!inline.contains("include ! ("), "{inline}");
    assert!(inline.contains("# [path = \""), "{inline}");

    // The folder's file is the folder, so a module pointing at nothing says so. The path is
    // the one the report names, or the report lookup would answer first — and it does not
    // exist, which is the mistake this message is about.
    let missing = expand_registry(
        "",
        "pub mod commands {}",
        "/nonexistent/src/commands",
        &report,
    );
    assert!(missing.contains("which does not exist"), "{missing}");
    assert!(missing.contains("path = "), "{missing}");
    // The folder it looked in is named, so the fix is visible without a guess. (It is the
    // absolute path here, because `CARGO_MANIFEST_DIR` is this crate's rather than the
    // application's — in a real build the same message is relative to the application.)
    assert!(
        missing.contains("/nonexistent/src/commands/mod.rs"),
        "{missing}"
    );
}

#[test]
fn a_registry_defaults_the_request_to_action_so_the_folder_may_keep_its_own() {
    // The registry's folder is where a hand-written `Request` lives, so the generated one has
    // a name of its own unless the call says otherwise.
    let default = registry("");
    assert!(default.contains("pub enum Action {"), "{default}");
    assert!(
        default.contains("TryFrom < Commands > for Action"),
        "{default}"
    );
    // An explicit `request = ..` wins, including the name `Request` itself.
    let named = registry("request = Chosen");
    assert!(named.contains("pub enum Chosen {"), "{named}");
    assert!(!named.contains("pub enum Action {"), "{named}");
    let request = registry("request = Request");
    assert!(request.contains("pub enum Request {"), "{request}");
}

#[test]
fn a_registry_forwards_its_keys_to_the_tree_and_the_root() {
    let tokens = registry("output = crate::Said, context = crate::App, name = \"discovered\"");
    assert!(
        tokens.contains("clifacet :: Handler < crate :: App > for Commands"),
        "{tokens}"
    );
    assert!(
        tokens.contains("name = \"discovered\"") && !tokens.contains("CARGO_PKG_NAME"),
        "{tokens}"
    );
    // A context needs an output, and giving both means the tree is a handler, not a runner.
    let plain = registry("output = crate::Said, planned");
    assert!(
        plain.contains("impl :: clifacet :: command :: Planned for Commands"),
        "{plain}"
    );
    let minimal = registry("output = crate::Said");
    assert!(
        minimal.contains("impl :: clifacet :: Command for Commands"),
        "{minimal}"
    );
}

#[test]
fn a_registry_takes_no_list_of_modules_and_says_why() {
    // The folder is the list, so a module name is not a key — and naming one is the mistake a
    // reader of `commands!(..)` makes first.
    let message = rejected_registry("list, echo");
    assert!(
        message.contains("`list` is not a key of `#[clifacet::registry(..)]`"),
        "{message}"
    );
    assert!(message.contains("the folder is the list"), "{message}");
    // `path` is the one key that names a place, and it is not a module name either.
    assert!(
        rejected_registry("echo = list").contains("is not a key"),
        "a value without a key is refused"
    );
}

#[test]
fn a_registry_refuses_a_module_under_another_name() {
    let (folder, report) = fixture();
    let message = expand_registry("", "pub mod store {}", &folder, &report);
    assert!(message.contains("is named `commands`"), "{message}");
    assert!(
        message.contains("call `clifacet::commands!(..)` inside it"),
        "the message says what to write instead: {message}"
    );
}

#[test]
fn a_registry_that_finds_no_folder_for_its_report_says_which_ones_there_are() {
    // A build script that scanned another folder is a one-line fix, so the message names both
    // and says where a registry looks.
    let (folder, _) = fixture();
    let report = "clifacet-commands/1;folder=src/elsewhere;command=other";
    let message = expand_registry("", "pub mod commands {}", &folder, report);
    assert!(
        message.contains("registers") && message.contains("the build script scanned"),
        "{message}"
    );
    assert!(message.contains("src/elsewhere"), "{message}");
    // And a report that is not the protocol at all is the other mistake: no build script.
    let message = expand_registry("", "pub mod commands {}", &folder, "whatever");
    assert!(message.contains("clifacet-commands/1"), "{message}");
}
