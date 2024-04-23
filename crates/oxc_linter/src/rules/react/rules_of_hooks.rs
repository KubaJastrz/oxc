use itertools::Itertools;
use oxc_ast::{ast::Function, AstKind};
use oxc_diagnostics::{
    miette::{self, Diagnostic},
    thiserror::Error,
};
use oxc_macros::declare_oxc_lint;
use oxc_semantic::{
    petgraph::{self, Direction},
    BasicBlockElement, Register,
};
use oxc_span::Span;

// TODO: REMOVE ME PLS
use std::dbg as std_dbg;
macro_rules! dbg {
    ($($any:tt)*) => (
        std_dbg!($($any)*)
    )
}

use crate::{
    context::LintContext,
    rule::Rule,
    utils::{is_react_component_name, is_react_hook_name},
    AstNode,
};

#[derive(Debug, Error, Diagnostic)]
#[error("eslint-plugin-react-hooks(rules-of-hooks): TODO")]
#[diagnostic(severity(warning), help("TODO"))]
enum RulesOfHooksDiagnostic {
    FunctionError(#[label] Span),
    ConditionalError(#[label] Span),
}

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
        let is_hook = call.callee_name().is_some_and(is_react_hook_name);

        if !is_hook {
            return;
        }

        let semantic = ctx.semantic();

        let is_func = |it: &AstNode| it.kind().is_function_like();

        let mut ancestors =
            semantic.nodes().ancestors(node.id()).map(|id| semantic.nodes().get_node(id));

        let parent = ancestors.next().unwrap();

        let parent_func =
            if is_func(parent) { parent } else { ancestors.find(|it| is_func(it)).unwrap() };

        match parent_func.kind() {
            AstKind::Function(Function { id: Some(id), .. })
                if !is_react_component_name(&id.name) && !is_react_hook_name(&id.name) =>
            {
                ctx.diagnostic(RulesOfHooksDiagnostic::FunctionError(id.span));
            }
            _ => {
                dbg!("TODO!");
            }
        }

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

        let Some((_, astar)) =
            petgraph::algo::astar(&cfg.graph, func_cfg_ix, |it| it == node_cfg_ix, |_| 0, |_| 0)
        else {
            // There should always be a control flow path between a parent and child node.
            // If there is none it means we always do an early exit before reaching our hook call.
            return;
        };

        let astar = astar.chunks(astar.len() - 1).next().unwrap();
        dbg!(&astar);

        let func_to_node_path_blocks = astar
            .iter()
            .flat_map(|c| {
                let blocks = cfg.basic_block_by_index(*c);
                blocks
            })
            .collect_vec();
        dbg!(&func_to_node_path_blocks);

        let func_to_node_all_edge_nodes =
            petgraph::algo::dijkstra(&cfg.graph, func_cfg_ix, Some(node_cfg_ix), |_| 0);
        dbg!(&func_to_node_all_edge_nodes);

        let mut all_edges_blocks = func_to_node_all_edge_nodes.keys().flat_map(|ix| {
            let blocks = cfg.basic_block_by_index(*ix);
            blocks
        });

        dbg!(&all_edges_blocks);

        if func_to_node_all_edge_nodes.len() == astar.len() {
            return;
        }

        if all_edges_blocks.any(|f| matches!(f, BasicBlockElement::Assignment(Register::Return, _)))
        {
            ctx.diagnostic(RulesOfHooksDiagnostic::ConditionalError(call.span));
        }

        // panic!();
    }
}

#[test]
fn test() {
    ///  Copyright (c) Meta Platforms, Inc. and affiliates.
    /// Most of these tests are sourced from the original react `eslint-plugin-react-hooks` package.
    /// https://github.com/facebook/react/blob/5b903cdaa94c78e8fabb985d8daca5bd7d266323/packages/eslint-plugin-react-hooks/__tests__/ESLintRulesOfHooks-test.js#L43
    use crate::tester::Tester;

    let pass = vec![
        // Valid because components can use hooks.
        "
            function ComponentWithHook() {
              useHook();
            }
        ",
        // Valid because components can use hooks.
        "
            function createComponentWithHook() {
              return function ComponentWithHook() {
                useHook();
              };
            }
        ",
        // Valid because hooks can use hooks.
        "
            function useHookWithHook() {
              useHook();
            }
        ",
        // Valid because hooks can use hooks.
        "
            function createHook() {
              return function useHookWithHook() {
                useHook();
              }
            }
        ",
        // Valid because components can call functions.
        "
            function ComponentWithNormalFunction() {
              doSomething();
            }
        ",
        // Valid because functions can call functions.
        "
            function normalFunctionWithNormalFunction() {
              doSomething();
            }
        ",
        // Valid because functions can call functions.
        "
            function normalFunctionWithConditionalFunction() {
              if (cond) {
                doSomething();
              }
            }
        ",
        // Valid because functions can call functions.
        "
            function functionThatStartsWithUseButIsntAHook() {
              if (cond) {
                userFetch();
              }
            }
        ",
        // Valid although unconditional return doesn't make sense and would fail other rules.
        // We could make it invalid but it doesn't matter.
        "
            function useUnreachable() {
              return;
              useHook();
            }
        ",
        // // Valid because hooks can call hooks.
        // "
        //     function useHook() { useState(); }
        //     const whatever = function useHook() { useState(); };
        //     const useHook1 = () => { useState(); };
        //     let useHook2 = () => useState();
        //     useHook2 = () => { useState(); };
        //     ({useHook: () => { useState(); }});
        //     ({useHook() { useState(); }});
        //     const {useHook3 = () => { useState(); }} = {};
        //     ({useHook = () => { useState(); }} = {});
        //     Namespace.useHook = () => { useState(); };
        // ",
        // Valid because hooks can call hooks.
        "
            function useHook() {
              useHook1();
              useHook2();
            }
        ",
        // Valid because hooks can call hooks.
        "
            function createHook() {
              return function useHook() {
                useHook1();
                useHook2();
              };
            }
        ",
        // Valid because hooks can call hooks.
        "
            function useHook() {
              useState() && a;
            }
        ",
        // Valid because hooks can call hooks.
        "
            function useHook() {
              return useHook1() + useHook2();
            }
        ",
        // Valid because hooks can call hooks.
        "
            function useHook() {
              return useHook1(useHook2());
            }
        ",
        // Valid because hooks can be used in anonymous arrow-function arguments
        // to forwardRef.
        "
            const FancyButton = React.forwardRef((props, ref) => {
              useHook();
              return <button {...props} ref={ref} />
            });
        ",
        // Valid because hooks can be used in anonymous function arguments to
        // forwardRef.
        "
            const FancyButton = React.forwardRef(function (props, ref) {
              useHook();
              return <button {...props} ref={ref} />
            });
        ",
        // Valid because hooks can be used in anonymous function arguments to
        // forwardRef.
        "
            const FancyButton = forwardRef(function (props, ref) {
              useHook();
              return <button {...props} ref={ref} />
            });
        ",
        // Valid because hooks can be used in anonymous function arguments to
        // React.memo.
        "
            const MemoizedFunction = React.memo(props => {
              useHook();
              return <button {...props} />
            });
        ",
        // Valid because hooks can be used in anonymous function arguments to
        // memo.
        "
            const MemoizedFunction = memo(function (props) {
              useHook();
              return <button {...props} />
            });
        ",
        // Valid because classes can call functions.
        // We don't consider these to be hooks.
        "
            class C {
              m() {
                this.useHook();
                super.useHook();
              }
            }
        ",
        // // Valid -- this is a regression test.
        // "
        //     jest.useFakeTimers();
        //     beforeEach(() => {
        //       jest.useRealTimers();
        //     })
        // ",
        // // Valid because they're not matching use[A-Z].
        // "
        //     fooState();
        //     _use();
        //     _useState();
        //     use_hook();
        //     // also valid because it's not matching the PascalCase namespace
        //     jest.useFakeTimer()
        // ",
        // Regression test for some internal code.
        // This shows how the "callback rule" is more relaxed,
        // and doesn't kick in unless we're confident we're in
        // a component or a hook.
        "
            function makeListener(instance) {
              each(pixelsWithInferredEvents, pixel => {
                if (useExtendedSelector(pixel.id) && extendedButton) {
                  foo();
                }
              });
            }
        ",
        // This is valid because "use"-prefixed functions called in
        // unnamed function arguments are not assumed to be hooks.
        "
            React.unknownFunction((foo, bar) => {
              if (foo) {
                useNotAHook(bar)
              }
            });
        ",
        // This is valid because "use"-prefixed functions called in
        // unnamed function arguments are not assumed to be hooks.
        "
            unknownFunction(function(foo, bar) {
              if (foo) {
                useNotAHook(bar)
              }
            });
        ",
        // Regression test for incorrectly flagged valid code.
        "
            function RegressionTest() {
              const foo = cond ? a : b;
              useState();
            }
        ",
        // Valid because exceptions abort rendering
        "
            function RegressionTest() {
              if (page == null) {
                throw new Error('oh no!');
              }
              useState();
            }
        ",
        // Valid because the loop doesn't change the order of hooks calls.
        "
            function RegressionTest() {
              const res = [];
              const additionalCond = true;
              for (let i = 0; i !== 10 && additionalCond; ++i ) {
                res.push(i);
              }
              React.useLayoutEffect(() => {});
            }
        ",
        // Is valid but hard to compute by brute-forcing
        "
            function MyComponent() {
              // 40 conditions
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}
              if (c) {} else {}

              // 10 hooks
              useHook();
              useHook();
              useHook();
              useHook();
              useHook();
              useHook();
              useHook();
              useHook();
              useHook();
              useHook();
            }
        ",
        // // Valid because the neither the conditions before or after the hook affect the hook call
        // // Failed prior to implementing BigInt because pathsFromStartToEnd and allPathsFromStartToEnd were too big and had rounding errors
        // "
        //     const useSomeHook = () => {};
        //
        //     const SomeName = () => {
        //       const filler = FILLER ?? FILLER ?? FILLER;
        //       const filler2 = FILLER ?? FILLER ?? FILLER;
        //       const filler3 = FILLER ?? FILLER ?? FILLER;
        //       const filler4 = FILLER ?? FILLER ?? FILLER;
        //       const filler5 = FILLER ?? FILLER ?? FILLER;
        //       const filler6 = FILLER ?? FILLER ?? FILLER;
        //       const filler7 = FILLER ?? FILLER ?? FILLER;
        //       const filler8 = FILLER ?? FILLER ?? FILLER;
        //
        //       useSomeHook();
        //
        //       if (anyConditionCanEvenBeFalse) {
        //         return null;
        //       }
        //
        //       return (
        //         <React.Fragment>
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //           {FILLER ? FILLER : FILLER}
        //         </React.Fragment>
        //       );
        //     };
        //     ",
        // Valid because the neither the condition nor the loop affect the hook call.
        "
            function App(props) {
              const someObject = {propA: true};
              for (const propName in someObject) {
                if (propName === true) {
                } else {
                }
              }
              const [myState, setMyState] = useState(null);
            }
        ",
        "
            function App() {
              const text = use(Promise.resolve('A'));
              return <Text text={text} />
            }
        ",
        "
            import * as React from 'react';
            function App() {
              if (shouldShowText) {
                const text = use(query);
                const data = React.use(thing);
                const data2 = react.use(thing2);
                return <Text text={text} />
              }
              return <Text text={shouldFetchBackupText ? use(backupQuery) : \"Nothing to see here\"} />
            }
        ",
        "
            function App() {
              let data = [];
              for (const query of queries) {
                const text = use(item);
                data.push(text);
              }
              return <Child data={data} />
            }
        ",
        "
            function App() {
              const data = someCallback((x) => use(x));
              return <Child data={data} />
            }
        ",
        // {
        //   code: normalizeIndent`
        //     export const notAComponent = () => {
        //        return () => {
        //         useState();
        //       }
        //     }
        //   `,
        //   // TODO: this should error but doesn't.
        //   // errors: [functionError('use', 'notAComponent')],
        // },
        // {
        //   code: normalizeIndent`
        //     export default () => {
        //       if (isVal) {
        //         useState(0);
        //       }
        //     }
        //   `,
        //   // TODO: this should error but doesn't.
        //   // errors: [genericError('useState')],
        // },
        // {
        //   code: normalizeIndent`
        //     function notAComponent() {
        //       return new Promise.then(() => {
        //         useState();
        //       });
        //     }
        //   `,
        //   // TODO: this should error but doesn't.
        //   // errors: [genericError('useState')],
        // },
    ];

    let fail = vec![
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        "
            function useHook() {
              if (a) return; 
              useState();
            }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        "
            function useHook() {
              if (a) return;
              if (b) {
                console.log('true');
              } else {
                console.log('false');
              }
              useState();
            }
        ",
        // Is valid but hard to compute by brute-forcing
        "
            function MyComponent() {
              // 40 conditions
              // if (c) {} else {}
              if (c) {} else { return; }

              useHook();
            }
        ",
    ];

    Tester::new(RulesOfHooks::NAME, pass, fail).test_and_snapshot();
}
