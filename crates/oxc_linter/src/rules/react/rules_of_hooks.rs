use oxc_diagnostics::{
    miette::{self, Diagnostic},
    thiserror::Error,
};
use oxc_macros::declare_oxc_lint;
use oxc_span::Span;

use crate::{context::LintContext, rule::Rule, AstNode};

#[derive(Debug, Error, Diagnostic)]
#[error("eslint-plugin-react-hooks(rules-of-hooks): TODO")]
#[diagnostic(severity(warning), help("TODO"))]
struct RulesOfHooksDiagnostic(#[label] pub Span);

#[derive(Debug, Default, Clone)]
pub struct RulesOfHooks;

declare_oxc_lint!(
    /// ### What it does
    ///
    /// TODO
    RulesOfHooks,
    correctness
);

impl Rule for RulesOfHooks {
    fn run<'a>(&self, _: &AstNode<'a>, _: &LintContext<'a>) {}
}

#[test]
fn test() {
    use crate::tester::Tester;

    let pass = vec![("<App />;", None)];

    let fail = vec![];

    Tester::new(RulesOfHooks::NAME, pass, fail).test_and_snapshot();
}
