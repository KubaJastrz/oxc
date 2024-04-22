use itertools::Itertools;
use oxc_ast::AstKind;
use oxc_diagnostics::{
    miette::{self, Diagnostic},
    thiserror::Error,
};
use oxc_macros::declare_oxc_lint;
use oxc_semantic::{
    petgraph::{
        self,
        visit::{EdgeRef, NodeRef},
        Direction,
    },
    pg::neighbors_filtered_by_edge_weight,
    BasicBlockElement, EdgeType, Register,
};
use oxc_span::Span;

use crate::{context::LintContext, rule::Rule, utils::is_react_hook_name, AstNode};

#[derive(Debug, Error, Diagnostic)]
#[error("eslint-plugin-react-hooks(rules-of-hooks): TODO")]
#[diagnostic(severity(warning), help("TODO"))]
struct RulesOfHooksDiagnostic(#[label] pub Span);

#[derive(Debug, Default, Clone)]
pub struct RulesOfHooks;

declare_oxc_lint!(
    /// ### What it does
    ///
    /// This enforcecs the Rules of Hooks
    ///
    /// <https://reactjs.org/docs/hooks-rules.html>
    ///
    RulesOfHooks,
    correctness
);

impl Rule for RulesOfHooks {
    fn run<'a>(&self, node: &AstNode<'a>, ctx: &LintContext<'a>) {
        let AstKind::CallExpression(call) = node.kind() else { return };
        let is_hook = call.callee_name().is_some_and(|name| is_react_hook_name(name));

        if !is_hook {
            return;
        }

        let semantic = ctx.semantic();

        let is_func = |it: &AstNode| matches!(it.kind(), AstKind::Function(_));

        let mut ancestors =
            semantic.nodes().ancestors(node.id()).map(|id| semantic.nodes().get_node(id));

        let parent = ancestors.next().unwrap();

        let parent_func =
            if is_func(parent) { parent } else { ancestors.find(|it| is_func(it)).unwrap() };

        // .find()
        // .and_then(|it| Some(semantic.nodes().get_node(it)))
        // .unwrap();

        let cfg = semantic.cfg();
        let node_cfg_ix = node.cfg_ix();
        let func_cfg_ix = parent_func.cfg_ix();

        let parent_cfg_ix =
            cfg.graph.neighbors_directed(node_cfg_ix, Direction::Incoming).next().unwrap();

        dbg!(&func_cfg_ix);
        dbg!(&node_cfg_ix);
        dbg!(&parent_cfg_ix);

        // there is no branch between us and our parent function
        // TODO: we still need to make sure our parent is a component function.
        if node_cfg_ix == func_cfg_ix {
            return;
        }

        let astar =
            petgraph::algo::astar(&cfg.graph, func_cfg_ix, |it| it == node_cfg_ix, |_| 0, |_| 0)
                .expect(
                    "There should always be a control flow path between a parent and child node.",
                )
                .1;

        let astar = astar.chunks(astar.len() - 1).next().unwrap();
        dbg!(&astar);

        let func_to_node_path = astar
            .iter()
            .map(|c| {
                let blocks = cfg.basic_block_by_index(*c);
                blocks
            })
            .flatten()
            .collect_vec();
        dbg!(&func_to_node_path);

        let func_to_node_all_edge_nodes =
            petgraph::algo::dijkstra(&cfg.graph, func_cfg_ix, Some(parent_cfg_ix), |_| 0);
        dbg!(&func_to_node_all_edge_nodes);

        let mut all_edges_blocks = func_to_node_all_edge_nodes
            .iter()
            .map(|(c, _)| {
                let blocks = cfg.basic_block_by_index(*c);
                blocks
            })
            .flatten();

        dbg!(&all_edges_blocks);

        if func_to_node_all_edge_nodes.len() == func_to_node_path.len() {
            return;
        }

        if all_edges_blocks.any(|f| matches!(f, BasicBlockElement::Assignment(Register::Return, _)))
        {
            ctx.diagnostic(RulesOfHooksDiagnostic(call.span));
        }

        // panic!();
    }
}

#[test]
fn test() {
    use crate::tester::Tester;

    let pass = vec!["<App />"];

    let fail = vec![
        // Is valid but hard to compute by brute-forcing
        "
        function MyComponent() {
          // 40 conditions
          // if (c) {} else {}
          if (c) {} else { return; }

          if (c) {
          useHook();

          } else {}
        }

        ",
    ];

    Tester::new(RulesOfHooks::NAME, pass, fail).test_and_snapshot();
}
