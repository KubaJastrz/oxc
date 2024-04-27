use itertools::Itertools;
use oxc_ast::{ast::Function, AstKind};
use oxc_diagnostics::{
    miette::{self, Diagnostic},
    thiserror::Error,
};
use oxc_macros::declare_oxc_lint;
use oxc_semantic::{
    petgraph::{self, Direction},
    AstNodeId, AstNodes, BasicBlockElement, Register,
};
use oxc_span::{GetSpan, Span};

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
enum RulesOfHooksDiagnostic {
    #[error(
        "eslint-plugin-react-hooks(rules-of-hooks): \
      React Hook \"{hook:?}\" is called in function \"{func:?}\" that is neither \
      a React function component nor a custom React Hook function. \
      React component names must start with an uppercase letter. \
      React Hook names must start with the word \"use\"."
    )]
    #[diagnostic(severity(warning), help("TODO"))]
    FunctionError {
        #[label]
        span: Span,
        #[label]
        hook: Span,
        #[label]
        func: Span,
    },
    #[error("eslint-plugin-react-hooks(rules-of-hooks): TODO: ConditionalError")]
    #[diagnostic(severity(warning), help("TODO: ConditionalError"))]
    ConditionalError(#[label] Span),
    #[error("eslint-plugin-react-hooks(rules-of-hooks): TODO: TopLevelError")]
    #[diagnostic(severity(warning), help("TODO: TopLevelError"))]
    TopLevelError(#[label] Span),
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

        let mut ancestors =
            semantic.nodes().ancestors(node.id()).map(|id| semantic.nodes().get_node(id));

        let Some(parent_func) = parent_func(semantic.nodes(), node) else {
            ctx.diagnostic(RulesOfHooksDiagnostic::TopLevelError(call.span));
            return;
        };

        match parent_func.kind() {
            AstKind::Function(Function { id: Some(id), .. })
                if !is_react_component_name(&id.name) && !is_react_hook_name(&id.name) =>
            {
                ctx.diagnostic(RulesOfHooksDiagnostic::FunctionError {
                    span: id.span,
                    hook: call.callee.span(),
                    func: id.span,
                });
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

        let func_to_node_all_edge_nodes =
            petgraph::algo::dijkstra(&cfg.graph, func_cfg_ix, Some(node_cfg_ix), |_| 0);
        dbg!(&func_to_node_all_edge_nodes);

        let all_edges_blocks = func_to_node_all_edge_nodes.keys().flat_map(|ix| {
            let blocks = cfg.basic_block_by_index(*ix);
            blocks
        });

        dbg!(&all_edges_blocks);

        if func_to_node_all_edge_nodes.len() == astar.len() {
            return;
        }

        // if all_edges_blocks.any(|f| matches!(f, BasicBlockElement::Assignment(Register::Return, _)))
        if func_to_node_all_edge_nodes
            .into_iter()
            .any(|(f, _)| !petgraph::algo::has_path_connecting(&cfg.graph, f, node_cfg_ix, None))
        {
            ctx.diagnostic(RulesOfHooksDiagnostic::ConditionalError(call.span));
        }

        // panic!();
    }
}

fn parent_func<'a>(nodes: &'a AstNodes<'a>, node: &AstNode) -> Option<&'a AstNode<'a>> {
    nodes.ancestors(node.id()).map(|id| nodes.get_node(id)).find(|it| it.kind().is_function_like())
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
        // Valid because hooks can call hooks.
        "
            function useHook() { useState(); }
            const whatever = function useHook() { useState(); };
            const useHook1 = () => { useState(); };
            let useHook2 = () => useState();
            useHook2 = () => { useState(); };
            ({useHook: () => { useState(); }});
            ({useHook() { useState(); }});
            const {useHook3 = () => { useState(); }} = {};
            ({useHook = () => { useState(); }} = {});
            Namespace.useHook = () => { useState(); };
        ",
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
        // TODO: jest cases do not work at the moment, FIX me!
        // Valid -- this is a regression test.
        // "
        //     jest.useFakeTimers();
        //     beforeEach(() => {
        //       jest.useRealTimers();
        //     })
        // ",
        // Valid because they're not matching use[A-Z].
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
        // "
        //     React.unknownFunction((foo, bar) => {
        //       if (foo) {
        //         useNotAHook(bar)
        //       }
        //     });
        // ",
        // This is valid because "use"-prefixed functions called in
        // unnamed function arguments are not assumed to be hooks.
        // "
        //     unknownFunction(function(foo, bar) {
        //       if (foo) {
        //         useNotAHook(bar)
        //       }
        //     });
        // ",
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
        // "
        //     function RegressionTest() {
        //       const res = [];
        //       const additionalCond = true;
        //       for (let i = 0; i !== 10 && additionalCond; ++i ) {
        //         res.push(i);
        //       }
        //       React.useLayoutEffect(() => {});
        //     }
        // ",
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
        // Valid because the neither the conditions before or after the hook affect the hook call
        // Failed prior to implementing BigInt because pathsFromStartToEnd and allPathsFromStartToEnd were too big and had rounding errors
        "
            const useSomeHook = () => {};

            const SomeName = () => {
              const filler = FILLER ?? FILLER ?? FILLER;
              const filler2 = FILLER ?? FILLER ?? FILLER;
              const filler3 = FILLER ?? FILLER ?? FILLER;
              const filler4 = FILLER ?? FILLER ?? FILLER;
              const filler5 = FILLER ?? FILLER ?? FILLER;
              const filler6 = FILLER ?? FILLER ?? FILLER;
              const filler7 = FILLER ?? FILLER ?? FILLER;
              const filler8 = FILLER ?? FILLER ?? FILLER;

              useSomeHook();

              if (anyConditionCanEvenBeFalse) {
                return null;
              }

              return (
                <React.Fragment>
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                  {FILLER ? FILLER : FILLER}
                </React.Fragment>
              );
            };
            ",
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
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [conditionalError('useConditionalHook')],
        "
        function ComponentWithConditionalHook() {
               if (cond) {
                 useConditionalHook();
               }
             }
        ",
        // Invalid because hooks can only be called inside of a component.
        // errors: [
        //     topLevelError('Hook.useState'),
        //     topLevelError('Hook.use42'),
        //     topLevelError('Hook.useHook'),
        // ],
        "
            Hook.useState();
            Hook._useState();
            Hook.use42();
            Hook.useHook();
            Hook.use_hook();
        ",
        //          {
        //            code: normalizeIndent`
        //              class C {
        //                m() {
        //                  This.useHook();
        //                  Super.useHook();
        //                }
        //              }
        //            `,
        //            errors: [classError('This.useHook'), classError('Super.useHook')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // This is a false positive (it's valid) that unfortunately
        //              // we cannot avoid. Prefer to rename it to not start with "use"
        //              class Foo extends Component {
        //                render() {
        //                  if (cond) {
        //                    FooStore.useFeatureFlag();
        //                  }
        //                }
        //              }
        //            `,
        //            errors: [classError('FooStore.useFeatureFlag')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function ComponentWithConditionalHook() {
        //                if (cond) {
        //                  Namespace.useConditionalHook();
        //                }
        //              }
        //            `,
        //            errors: [conditionalError('Namespace.useConditionalHook')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function createComponent() {
        //                return function ComponentWithConditionalHook() {
        //                  if (cond) {
        //                    useConditionalHook();
        //                  }
        //                }
        //              }
        //            `,
        //            errors: [conditionalError('useConditionalHook')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function useHookWithConditionalHook() {
        //                if (cond) {
        //                  useConditionalHook();
        //                }
        //              }
        //            `,
        //            errors: [conditionalError('useConditionalHook')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function createHook() {
        //                return function useHookWithConditionalHook() {
        //                  if (cond) {
        //                    useConditionalHook();
        //                  }
        //                }
        //              }
        //            `,
        //            errors: [conditionalError('useConditionalHook')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function ComponentWithTernaryHook() {
        //                cond ? useTernaryHook() : null;
        //              }
        //            `,
        //            errors: [conditionalError('useTernaryHook')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's a common misunderstanding.
        //              // We *could* make it valid but the runtime error could be confusing.
        //              function ComponentWithHookInsideCallback() {
        //                useEffect(() => {
        //                  useHookInsideCallback();
        //                });
        //              }
        //            `,
        //            errors: [genericError('useHookInsideCallback')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's a common misunderstanding.
        //              // We *could* make it valid but the runtime error could be confusing.
        //              function createComponent() {
        //                return function ComponentWithHookInsideCallback() {
        //                  useEffect(() => {
        //                    useHookInsideCallback();
        //                  });
        //                }
        //              }
        //            `,
        //            errors: [genericError('useHookInsideCallback')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's a common misunderstanding.
        //              // We *could* make it valid but the runtime error could be confusing.
        //              const ComponentWithHookInsideCallback = React.forwardRef((props, ref) => {
        //                useEffect(() => {
        //                  useHookInsideCallback();
        //                });
        //                return <button {...props} ref={ref} />
        //              });
        //            `,
        //            errors: [genericError('useHookInsideCallback')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's a common misunderstanding.
        //              // We *could* make it valid but the runtime error could be confusing.
        //              const ComponentWithHookInsideCallback = React.memo(props => {
        //                useEffect(() => {
        //                  useHookInsideCallback();
        //                });
        //                return <button {...props} />
        //              });
        //            `,
        //            errors: [genericError('useHookInsideCallback')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's a common misunderstanding.
        //              // We *could* make it valid but the runtime error could be confusing.
        //              function ComponentWithHookInsideCallback() {
        //                function handleClick() {
        //                  useState();
        //                }
        //              }
        //            `,
        //            errors: [functionError('useState', 'handleClick')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's a common misunderstanding.
        //              // We *could* make it valid but the runtime error could be confusing.
        //              function createComponent() {
        //                return function ComponentWithHookInsideCallback() {
        //                  function handleClick() {
        //                    useState();
        //                  }
        //                }
        //              }
        //            `,
        //            errors: [functionError('useState', 'handleClick')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function ComponentWithHookInsideLoop() {
        //                while (cond) {
        //                  useHookInsideLoop();
        //                }
        //              }
        //            `,
        //            errors: [loopError('useHookInsideLoop')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function renderItem() {
        //                useState();
        //              }
        //
        //              function List(props) {
        //                return props.items.map(renderItem);
        //              }
        //            `,
        //            errors: [functionError('useState', 'renderItem')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Currently invalid because it violates the convention and removes the "taint"
        //              // from a hook. We *could* make it valid to avoid some false positives but let's
        //              // ensure that we don't break the "renderItem" and "normalFunctionWithConditionalHook"
        //              // cases which must remain invalid.
        //              function normalFunctionWithHook() {
        //                useHookInsideNormalFunction();
        //              }
        //            `,
        //            errors: [
        //              functionError('useHookInsideNormalFunction', 'normalFunctionWithHook'),
        //            ],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // These are neither functions nor hooks.
        //              function _normalFunctionWithHook() {
        //                useHookInsideNormalFunction();
        //              }
        //              function _useNotAHook() {
        //                useHookInsideNormalFunction();
        //              }
        //            `,
        //            errors: [
        //              functionError('useHookInsideNormalFunction', '_normalFunctionWithHook'),
        //              functionError('useHookInsideNormalFunction', '_useNotAHook'),
        //            ],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function normalFunctionWithConditionalHook() {
        //                if (cond) {
        //                  useHookInsideNormalFunction();
        //                }
        //              }
        //            `,
        //            errors: [
        //              functionError(
        //                'useHookInsideNormalFunction',
        //                'normalFunctionWithConditionalHook'
        //              ),
        //            ],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function useHookInLoops() {
        //                while (a) {
        //                  useHook1();
        //                  if (b) return;
        //                  useHook2();
        //                }
        //                while (c) {
        //                  useHook3();
        //                  if (d) return;
        //                  useHook4();
        //                }
        //              }
        //            `,
        //            errors: [
        //              loopError('useHook1'),
        //              loopError('useHook2'),
        //              loopError('useHook3'),
        //              loopError('useHook4'),
        //            ],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function useHookInLoops() {
        //                while (a) {
        //                  useHook1();
        //                  if (b) continue;
        //                  useHook2();
        //                }
        //              }
        //            `,
        //            errors: [loopError('useHook1'), loopError('useHook2', true)],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function useLabeledBlock() {
        //                label: {
        //                  if (a) break label;
        //                  useHook();
        //                }
        //              }
        //            `,
        //            errors: [conditionalError('useHook')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Currently invalid.
        //              // These are variations capturing the current heuristic--
        //              // we only allow hooks in PascalCase or useFoo functions.
        //              // We *could* make some of these valid. But before doing it,
        //              // consider specific cases documented above that contain reasoning.
        //              function a() { useState(); }
        //              const whatever = function b() { useState(); };
        //              const c = () => { useState(); };
        //              let d = () => useState();
        //              e = () => { useState(); };
        //              ({f: () => { useState(); }});
        //              ({g() { useState(); }});
        //              const {j = () => { useState(); }} = {};
        //              ({k = () => { useState(); }} = {});
        //            `,
        //            errors: [
        //              functionError('useState', 'a'),
        //              functionError('useState', 'b'),
        //              functionError('useState', 'c'),
        //              functionError('useState', 'd'),
        //              functionError('useState', 'e'),
        //              functionError('useState', 'f'),
        //              functionError('useState', 'g'),
        //              functionError('useState', 'j'),
        //              functionError('useState', 'k'),
        //            ],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function useHook() {
        //                if (a) return;
        //                useState();
        //              }
        //            `,
        //            errors: [conditionalError('useState', true)],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function useHook() {
        //                if (a) return;
        //                if (b) {
        //                  console.log('true');
        //                } else {
        //                  console.log('false');
        //                }
        //                useState();
        //              }
        //            `,
        //            errors: [conditionalError('useState', true)],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function useHook() {
        //                if (b) {
        //                  console.log('true');
        //                } else {
        //                  console.log('false');
        //                }
        //                if (a) return;
        //                useState();
        //              }
        //            `,
        //            errors: [conditionalError('useState', true)],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function useHook() {
        //                a && useHook1();
        //                b && useHook2();
        //              }
        //            `,
        //            errors: [conditionalError('useHook1'), conditionalError('useHook2')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function useHook() {
        //                try {
        //                  f();
        //                  useState();
        //                } catch {}
        //              }
        //            `,
        //            errors: [
        //              // NOTE: This is an error since `f()` could possibly throw.
        //              conditionalError('useState'),
        //            ],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              function useHook({ bar }) {
        //                let foo1 = bar && useState();
        //                let foo2 = bar || useState();
        //                let foo3 = bar ?? useState();
        //              }
        //            `,
        //            errors: [
        //              conditionalError('useState'),
        //              conditionalError('useState'),
        //              conditionalError('useState'),
        //            ],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              const FancyButton = React.forwardRef((props, ref) => {
        //                if (props.fancy) {
        //                  useCustomHook();
        //                }
        //                return <button ref={ref}>{props.children}</button>;
        //              });
        //            `,
        //            errors: [conditionalError('useCustomHook')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              const FancyButton = forwardRef(function(props, ref) {
        //                if (props.fancy) {
        //                  useCustomHook();
        //                }
        //                return <button ref={ref}>{props.children}</button>;
        //              });
        //            `,
        //            errors: [conditionalError('useCustomHook')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous and might not warn otherwise.
        //              // This *must* be invalid.
        //              const MemoizedButton = memo(function(props) {
        //                if (props.fancy) {
        //                  useCustomHook();
        //                }
        //                return <button>{props.children}</button>;
        //              });
        //            `,
        //            errors: [conditionalError('useCustomHook')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // This is invalid because "use"-prefixed functions used in named
        //              // functions are assumed to be hooks.
        //              React.unknownFunction(function notAComponent(foo, bar) {
        //                useProbablyAHook(bar)
        //              });
        //            `,
        //            errors: [functionError('useProbablyAHook', 'notAComponent')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Invalid because it's dangerous.
        //              // Normally, this would crash, but not if you use inline requires.
        //              // This *must* be invalid.
        //              // It's expected to have some false positives, but arguably
        //              // they are confusing anyway due to the use*() convention
        //              // already being associated with Hooks.
        //              useState();
        //              if (foo) {
        //                const foo = React.useCallback(() => {});
        //              }
        //              useCustomHook();
        //            `,
        //            errors: [
        //              topLevelError('useState'),
        //              topLevelError('React.useCallback'),
        //              topLevelError('useCustomHook'),
        //            ],
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Technically this is a false positive.
        //              // We *could* make it valid (and it used to be).
        //              //
        //              // However, top-level Hook-like calls can be very dangerous
        //              // in environments with inline requires because they can mask
        //              // the runtime error by accident.
        //              // So we prefer to disallow it despite the false positive.
        //
        //              const {createHistory, useBasename} = require('history-2.1.2');
        //              const browserHistory = useBasename(createHistory)({
        //                basename: '/',
        //              });
        //            `,
        //            errors: [topLevelError('useBasename')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              class ClassComponentWithFeatureFlag extends React.Component {
        //                render() {
        //                  if (foo) {
        //                    useFeatureFlag();
        //                  }
        //                }
        //              }
        //            `,
        //            errors: [classError('useFeatureFlag')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              class ClassComponentWithHook extends React.Component {
        //                render() {
        //                  React.useState();
        //                }
        //              }
        //            `,
        //            errors: [classError('React.useState')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              (class {useHook = () => { useState(); }});
        //            `,
        //            errors: [classError('useState')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              (class {useHook() { useState(); }});
        //            `,
        //            errors: [classError('useState')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              (class {h = () => { useState(); }});
        //            `,
        //            errors: [classError('useState')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              (class {i() { useState(); }});
        //            `,
        //            errors: [classError('useState')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              async function AsyncComponent() {
        //                useState();
        //              }
        //            `,
        //            errors: [asyncComponentHookError('useState')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              async function useAsyncHook() {
        //                useState();
        //              }
        //            `,
        //            errors: [asyncComponentHookError('useState')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              Hook.use();
        //              Hook._use();
        //              Hook.useState();
        //              Hook._useState();
        //              Hook.use42();
        //              Hook.useHook();
        //              Hook.use_hook();
        //            `,
        //            errors: [
        //              topLevelError('Hook.use'),
        //              topLevelError('Hook.useState'),
        //              topLevelError('Hook.use42'),
        //              topLevelError('Hook.useHook'),
        //            ],
        //          },
        //          {
        //            code: normalizeIndent`
        //              function notAComponent() {
        //                use(promise);
        //              }
        //            `,
        //            errors: [functionError('use', 'notAComponent')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              const text = use(promise);
        //              function App() {
        //                return <Text text={text} />
        //              }
        //            `,
        //            errors: [topLevelError('use')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              class C {
        //                m() {
        //                  use(promise);
        //                }
        //              }
        //            `,
        //            errors: [classError('use')],
        //          },
        //          {
        //            code: normalizeIndent`
        //              async function AsyncComponent() {
        //                use();
        //              }
        //            `,
        //            errors: [asyncComponentHookError('use')],
        //          },
        //   ]     ,
        // };
        //
        // if      (__EXPERIMENTAL__) {
        //   t     ests.valid = [
        //          ...tests.valid,
        //          {
        //            code: normalizeIndent`
        //              // Valid because functions created with useEffectEvent can be called in a useEffect.
        //              function MyComponent({ theme }) {
        //                const onClick = useEffectEvent(() => {
        //                  showNotification(theme);
        //                });
        //                useEffect(() => {
        //                  onClick();
        //                });
        //              }
        //            `,
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Valid because functions created with useEffectEvent can be called in closures.
        //              function MyComponent({ theme }) {
        //                const onClick = useEffectEvent(() => {
        //                  showNotification(theme);
        //                });
        //                return <Child onClick={() => onClick()}></Child>;
        //              }
        //            `,
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Valid because functions created with useEffectEvent can be called in closures.
        //              function MyComponent({ theme }) {
        //                const onClick = useEffectEvent(() => {
        //                  showNotification(theme);
        //                });
        //                const onClick2 = () => { onClick() };
        //                const onClick3 = useCallback(() => onClick(), []);
        //                return <>
        //                  <Child onClick={onClick2}></Child>
        //                  <Child onClick={onClick3}></Child>
        //                </>;
        //              }
        //            `,
        //          },
        //          {
        //            code: normalizeIndent`
        //              // Valid because functions created with useEffectEvent can be passed by reference in useEffect
        //              // and useEffectEvent.
        //              function MyComponent({ theme }) {
        //                const onClick = useEffectEvent(() => {
        //                  showNotification(theme);
        //                });
        //                const onClick2 = useEffectEvent(() => {
        //                  debounce(onClick);
        //                });
        //                useEffect(() => {
        //                  let id = setInterval(onClick, 100);
        //                  return () => clearInterval(onClick);
        //                }, []);
        //                return <Child onClick={() => onClick2()} />
        //              }
        //            `,
        //          },
        //          {
        //            code: normalizeIndent`
        //              const MyComponent = ({theme}) => {
        //                const onClick = useEffectEvent(() => {
        //                  showNotification(theme);
        //                });
        //                return <Child onClick={() => onClick()}></Child>;
        //              };
        //            `,
        //          },
        //          {
        //            code: normalizeIndent`
        //              function MyComponent({ theme }) {
        //                const notificationService = useNotifications();
        //                const showNotification = useEffectEvent((text) => {
        //                  notificationService.notify(theme, text);
        //                });
        //                const onClick = useEffectEvent((text) => {
        //                  showNotification(text);
        //                });
        //                return <Child onClick={(text) => onClick(text)} />
        //              }
        //            `,
        //          },
        //          {
        //            code: normalizeIndent`
        //              function MyComponent({ theme }) {
        //                useEffect(() => {
        //                  onClick();
        //                });
        //                const onClick = useEffectEvent(() => {
        //                  showNotification(theme);
        //                });
        //              }
        //            `,
        //          },
    ];

    Tester::new(RulesOfHooks::NAME, pass, fail).test_and_snapshot();
}
