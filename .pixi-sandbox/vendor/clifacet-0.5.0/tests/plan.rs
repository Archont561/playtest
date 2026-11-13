//! A command whose run is a value first: described by a dry run, executed by a real
//! one, and with one execution path between them (decision-18 item 5).
//!
//! `Planned` commands get their `Command` impl from the library, and that impl is
//! `plan(input)?.execute()` — so there is no second way to run one, and a dry run that
//! describes the plan describes exactly what would execute.

use std::cell::RefCell;
use std::rc::Rc;

use clifacet::command::{Plan, Planned};
use clifacet::harness::{Outcome, Parity, Subject};
use clifacet::{Command, Failure, FailureKind};
use rstest::rstest;

/// Where a deployment's effects land: a log of what was actually done.
type World = Rc<RefCell<Vec<String>>>;

/// Deploys each named service, or refuses an empty list before doing anything.
struct Deploy {
    world: World,
    /// The description a drifting implementation would print instead of the truth.
    claims: Option<&'static str>,
}

impl Deploy {
    fn honest(world: &World) -> Self {
        Self {
            world: Rc::clone(world),
            claims: None,
        }
    }
}

impl Planned for Deploy {
    type Input = Vec<String>;
    type Output = usize;

    fn plan(&self, services: Vec<String>) -> Result<Plan<'_, usize>, Failure> {
        if services.is_empty() {
            return Err(Failure::validation("name at least one service"));
        }
        let steps: Vec<String> = services
            .iter()
            .map(|service| {
                self.claims
                    .map_or_else(|| format!("deploy {service}"), str::to_owned)
            })
            .collect();
        let world = Rc::clone(&self.world);
        let plan = Plan::new(move || {
            for service in &services {
                world.borrow_mut().push(format!("deploy {service}"));
            }
            Ok(services.len())
        });
        Ok(steps.into_iter().fold(plan, Plan::step))
    }
}

fn services(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn describing_a_plan_does_nothing() {
    let world = World::default();
    let deploy = Deploy::honest(&world);

    let plan = deploy
        .plan(services(&["web", "db"]))
        .expect("a valid request");

    assert_eq!(plan.steps(), ["deploy web", "deploy db"]);
    assert_eq!(plan.describe(), "deploy web\ndeploy db");
    assert!(
        world.borrow().is_empty(),
        "nothing ran: {:?}",
        world.borrow()
    );
}

#[test]
fn executing_the_plan_is_running_the_command() {
    let world = World::default();
    let deploy = Deploy::honest(&world);

    let deployed = deploy.execute(services(&["web"])).expect("runs");

    assert_eq!(deployed, 1);
    assert_eq!(*world.borrow(), ["deploy web"]);
}

#[test]
fn a_refused_plan_runs_nothing_either_way() {
    let world = World::default();
    let deploy = Deploy::honest(&world);

    let planned = deploy
        .plan(Vec::new())
        .expect_err("an empty list is refused");
    let executed = deploy.execute(Vec::new()).expect_err("by the same plan");

    assert_eq!(planned.kind(), FailureKind::Validation);
    assert_eq!(executed, planned);
    assert!(world.borrow().is_empty());
}

/// What a dry run says it would do, as an outcome the harness can compare.
fn described(deploy: &Deploy, input: Vec<String>) -> Outcome {
    let plan = deploy.plan(input).expect("a valid request");
    let steps = plan.steps().to_vec();
    Outcome::measured(|out, _err| {
        let _ = std::io::Write::write_all(out, plan.describe().as_bytes());
        0
    })
    .with_effects(steps)
}

/// What a real run did, as an outcome the harness can compare.
fn executed(deploy: &Deploy, world: &World, input: Vec<String>) -> Outcome {
    let code = i32::from(deploy.execute(input).is_err());
    Outcome::measured(|_out, _err| code).with_effects(world.borrow().clone())
}

#[rstest]
#[case::one(&["web"])]
#[case::several(&["web", "db", "cache"])]
fn a_described_plan_and_an_executed_plan_agree(#[case] names: &[&str]) {
    let world = World::default();
    let deploy = Deploy::honest(&world);

    Parity::new("described", described(&deploy, services(names)))
        .comparing([Subject::Exit, Subject::Effects])
        .compare("executed", executed(&deploy, &world, services(names)))
        .assert_parity();
}

/// The harness is what catches a description that drifted from the execution.
#[test]
fn a_plan_that_describes_something_else_is_caught() {
    let world = World::default();
    let drifting = Deploy {
        world: Rc::clone(&world),
        claims: Some("restart everything"),
    };

    let verdict = Parity::new("described", described(&drifting, services(&["web"])))
        .comparing([Subject::Effects])
        .compare("executed", executed(&drifting, &world, services(&["web"])))
        .check();

    let differences = verdict.expect_err("the description is not what ran");
    assert!(
        differences
            .iter()
            .any(|line| line.contains("restart everything")),
        "{differences:?}"
    );
}

#[test]
fn a_plan_shows_its_steps_but_not_its_closure() {
    let world = World::default();
    let deploy = Deploy::honest(&world);
    let plan = deploy.plan(services(&["web"])).expect("valid");

    assert_eq!(format!("{plan:?}"), r#"Plan { steps: ["deploy web"], .. }"#);
}
