//! The derive's diagnostics are part of its contract.
//!
//! A companion derive that fails obscurely is worse than no derive: the point is
//! to prove an *existing* clap command, so the errors it can raise have to say
//! what to write instead.
#[test]
fn the_command_attribute_wraps_clap_and_reports_what_it_cannot_wrap() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/command_valid.rs");
    cases.compile_fail("tests/ui/command_missing_input.rs");
    cases.compile_fail("tests/ui/command_on_enum.rs");
}

#[test]
fn a_function_command_reports_what_it_cannot_be() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/function_valid.rs");
    cases.compile_fail("tests/ui/function_not_result.rs");
    cases.compile_fail("tests/ui/function_pattern.rs");
    cases.compile_fail("tests/ui/function_reference.rs");
    cases.compile_fail("tests/ui/function_context_not_last.rs");
    cases.compile_fail("tests/ui/function_context_marked.rs");
    cases.compile_fail("tests/ui/function_flag_without_help.rs");
    cases.compile_fail("tests/ui/function_bridge_key.rs");
    cases.compile_fail("tests/ui/function_skip_with_rule.rs");
}

#[test]
fn the_facet_derive_reports_what_it_cannot_bridge() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/facet_valid.rs");
    cases.pass("tests/ui/facet_valid_bridge.rs");
    cases.compile_fail("tests/ui/facet_missing_input.rs");
    cases.compile_fail("tests/ui/facet_unknown_key.rs");
    cases.compile_fail("tests/ui/facet_on_enum.rs");
    cases.compile_fail("tests/ui/facet_missing_field.rs");
}

#[test]
fn a_field_attribute_declares_an_argument_and_reports_what_it_cannot_declare() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/field_attribute_valid.rs");
    cases.compile_fail("tests/ui/field_attribute_unknown_key.rs");
    cases.compile_fail("tests/ui/field_attribute_repeated_key.rs");
}

#[test]
fn a_field_rule_on_the_wrong_field_fails_to_compile_and_says_why() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/field_confirm_not_bool.rs");
    cases.compile_fail("tests/ui/field_ask_not_option.rs");
    cases.compile_fail("tests/ui/field_ask_secret.rs");
    cases.compile_fail("tests/ui/facet_secret_env_shown.rs");
    cases.compile_fail("tests/ui/field_ask_positional.rs");
    cases.compile_fail("tests/ui/field_rule_wrong_type.rs");
    cases.compile_fail("tests/ui/field_rule_beside_bridge.rs");
    cases.compile_fail("tests/ui/field_form_positional.rs");
    cases.compile_fail("tests/ui/field_form_list.rs");
    cases.compile_fail("tests/ui/field_skip_with_rule.rs");
    cases.compile_fail("tests/ui/facet_check_beside_bridge.rs");
}

#[test]
fn an_input_type_the_generated_impls_cannot_name_is_rejected_at_its_tokens() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/input_names_self.rs");
    cases.compile_fail("tests/ui/input_undeclared_lifetime.rs");
    cases.compile_fail("tests/ui/input_elided_lifetime.rs");
    cases.compile_fail("tests/ui/input_inferred.rs");
    cases.compile_fail("tests/ui/input_impl_trait.rs");
    cases.compile_fail("tests/ui/input_not_a_struct.rs");
    cases.compile_fail("tests/ui/input_not_a_type.rs");
}

#[test]
fn a_command_tree_is_declared_where_its_commands_live_and_says_what_it_cannot_declare() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/tree_valid.rs");
    // The tree's own surface: shapes and keys it cannot honour.
    cases.compile_fail("tests/ui/tree_inline_variant.rs");
    cases.compile_fail("tests/ui/tree_unit_variant.rs");
    cases.compile_fail("tests/ui/tree_repeated_name.rs");
    cases.compile_fail("tests/ui/tree_unknown_key.rs");
    cases.compile_fail("tests/ui/tree_variant_unknown_key.rs");
    cases.compile_fail("tests/ui/tree_planned_without_output.rs");
    // `Request` is the application's: what it gets wrong fails to compile.
    cases.compile_fail("tests/ui/tree_request_missing_variant.rs");
    cases.compile_fail("tests/ui/tree_request_extra_variant.rs");
    cases.compile_fail("tests/ui/tree_request_mistyped_variant.rs");
    // The leaves and the fields that hold a tree.
    cases.compile_fail("tests/ui/tree_leaf_without_input.rs");
    cases.compile_fail("tests/ui/tree_output_not_convertible.rs");
    cases.compile_fail("tests/ui/field_subcommands_not_a_tree.rs");
    cases.compile_fail("tests/ui/field_subcommands_with_rule.rs");
    cases.compile_fail("tests/ui/facet_collect_beside_ask.rs");
}

#[test]
fn a_handler_conflicts_with_a_command_and_planned_bridges_only_planned_with() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/handler_beside_command.rs");
    cases.compile_fail("tests/ui/planned_on_other_impl.rs");
    cases.compile_fail("tests/ui/planned_on_struct.rs");
}

/// `clifacet::commands!` reports what it cannot register, and says what to write instead.
#[test]
fn the_commands_macro_reports_what_it_cannot_register() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/commands_duplicate.rs");
    // A module that does not hold the struct its name promises (also the discovery path,
    // where the module was found rather than listed — CLI-50, AC#4).
    cases.compile_fail("tests/ui/commands_module_without_struct.rs");
    cases.compile_fail("tests/ui/commands_missing_list.rs");
    cases.compile_fail("tests/ui/commands_unknown_key.rs");
    cases.compile_fail("tests/ui/commands_planned_with_context.rs");
    cases.compile_fail("tests/ui/commands_on_other_attribute.rs");
}
