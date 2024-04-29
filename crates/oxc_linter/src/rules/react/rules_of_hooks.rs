use oxc_ast::{ast::Function, AstKind};
use oxc_diagnostics::{
    miette::{self, Diagnostic},
    thiserror::Error,
};
use oxc_macros::declare_oxc_lint;
use oxc_semantic::{petgraph, AstNodeId, AstNodes, EdgeType};
use oxc_span::{Atom, GetSpan, Span};

use crate::{
    context::LintContext,
    rule::Rule,
    utils::{is_react_component_name, is_react_hook, is_react_hook_name},
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
    #[diagnostic(severity(warning), help("TODO: FunctionError"))]
    FunctionError {
        #[label]
        span: Span,
        #[label]
        hook: Span,
        #[label]
        func: Span,
    },
    #[error("eslint-plugin-react-hooks(rules-of-hooks): TODO: ConditionalHook")]
    #[diagnostic(severity(warning), help("TODO: ConditionalHook"))]
    ConditionalHook(#[label] Span),
    #[error("eslint-plugin-react-hooks(rules-of-hooks): TODO: LoopHook")]
    #[diagnostic(severity(warning), help("TODO: LoopHook"))]
    LoopHook(#[label] Span),
    #[error("eslint-plugin-react-hooks(rules-of-hooks): TODO: TopLevelHook")]
    #[diagnostic(severity(warning), help("TODO: TopLevelHook"))]
    TopLevelHook(#[label] Span),
    #[error("eslint-plugin-react-hooks(rules-of-hooks): TODO: AsyncComponent")]
    #[diagnostic(severity(warning), help("TODO: AsyncComponent"))]
    AsyncComponent(#[label] Span),
    #[error("eslint-plugin-react-hooks(rules-of-hooks): TODO: AsyncComponent")]
    #[diagnostic(severity(warning), help("TODO: ClassComponent"))]
    ClassComponent(#[label] Span),
    #[error("eslint-plugin-react-hooks(rules-of-hooks): TODO: GenericError")]
    #[diagnostic(severity(warning), help("TODO: GenericError"))]
    GenericError(#[label] Span),
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

        if !is_react_hook(&call.callee) {
            return;
        }

        let semantic = ctx.semantic();
        let nodes = semantic.nodes();

        let Some(parent_func) = parent_func(nodes, node) else {
            return ctx.diagnostic(RulesOfHooksDiagnostic::TopLevelHook(call.span));
        };

        // Check if our parent function is part of a class.
        if matches!(
            nodes.parent_kind(parent_func.id()),
            Some(
                AstKind::MethodDefinition(_)
                    | AstKind::StaticBlock(_)
                    | AstKind::PropertyDefinition(_)
            )
        ) {
            return ctx.diagnostic(RulesOfHooksDiagnostic::ClassComponent(call.span));
        }

        let is_use = call.callee_name().is_some_and(|name| name == "use");

        match parent_func.kind() {
            AstKind::Function(Function { id: Some(id), .. })
                if !is_react_component_name(&id.name) && !is_react_hook_name(&id.name) =>
            {
                return ctx.diagnostic(RulesOfHooksDiagnostic::FunctionError {
                    span: id.span,
                    hook: call.callee.span(),
                    func: id.span,
                });
            }
            AstKind::Function(Function { id: Some(id), r#async: true, .. }) => {
                return ctx.diagnostic(RulesOfHooksDiagnostic::AsyncComponent(id.span));
            }
            // Hooks are allowed inside of unnamed functions used as arguments. As long as they are
            // not used as a callback inside of components or hooks.
            AstKind::Function(Function { id: None, .. }) | AstKind::ArrowFunctionExpression(_)
                if is_non_react_func_arg(nodes, parent_func.id()) =>
            {
                if !is_use && is_somewhere_inside_component_or_hook(nodes, parent_func.id()) {
                    ctx.diagnostic(RulesOfHooksDiagnostic::GenericError(call.span));
                }
                return;
            }
            _ => {}
        }

        // `use(...)` can be called conditionally, And,
        // `use(...)` can be called within a loop.
        // So we don't need the following checks.
        if is_use {
            return;
        }

        let graph = &semantic.cfg().graph;
        let node_cfg_ix = node.cfg_ix();
        let func_cfg_ix = parent_func.cfg_ix();

        // there is no branch between us and our parent function
        if node_cfg_ix == func_cfg_ix {
            return;
        }

        if !petgraph::algo::has_path_connecting(graph, func_cfg_ix, node_cfg_ix, None) {
            // There should always be a control flow path between a parent and child node.
            // If there is none it means we always do an early exit before reaching our hook call.
            // In some cases it might mean that we are operating on an invalid `cfg` but in either
            // case, It is somebody else's problem so we just return.
            return;
        }

        // TODO: all `dijkstra` algorithms can be merged together for better performance.
        let dijkstra = petgraph::algo::dijkstra(graph, func_cfg_ix, Some(node_cfg_ix), |_| 0);

        // Is this node cyclic?
        if petgraph::algo::dijkstra(graph, node_cfg_ix, None, |_| 0)
            .into_keys()
            .flat_map(|it| graph.edges_directed(it, petgraph::Direction::Outgoing))
            .any(|edge| matches!(edge.weight(), EdgeType::Backedge))
        {
            return ctx.diagnostic(RulesOfHooksDiagnostic::LoopHook(call.span));
        }

        // All nodes should be reachable from our hook, Otherwise we have a conditional/branching flow.
        if dijkstra
            .into_iter()
            .any(|(f, _)| !petgraph::algo::has_path_connecting(graph, f, node_cfg_ix, None))
        {
            #[allow(clippy::needless_return)]
            return ctx.diagnostic(RulesOfHooksDiagnostic::ConditionalHook(call.span));
        }
    }
}

fn parent_func<'a>(nodes: &'a AstNodes<'a>, node: &AstNode) -> Option<&'a AstNode<'a>> {
    nodes.ancestors(node.id()).map(|id| nodes.get_node(id)).find(|it| it.kind().is_function_like())
}

/// Checks if the `node_id` is a callback argument,
/// And that function isn't a `React.memo` or `React.forwardRef`.
/// Returns `true` if this node is a function argument and that isn't a React special function.
/// Otherwise it would return `false`.
fn is_non_react_func_arg(nodes: &AstNodes, node_id: AstNodeId) -> bool {
    let argument = match nodes.parent_node(node_id) {
        Some(parent) if matches!(parent.kind(), AstKind::Argument(_)) => parent,
        _ => return false,
    };

    let Some(AstKind::CallExpression(call)) = nodes.parent_kind(argument.id()) else {
        return false;
    };

    // TODO make it better, might have false positives.
    call.callee_name().is_some_and(|name| !matches!(name, "forwardRef" | "memo"))
}

fn is_somewhere_inside_component_or_hook(nodes: &AstNodes, node_id: AstNodeId) -> bool {
    nodes
        .ancestors(node_id)
        .map(|id| nodes.get_node(id))
        .filter(|node| node.kind().is_function_like())
        .map(|node| {
            (
                node.id(),
                match node.kind() {
                    AstKind::Function(func) => func.id.as_ref().map(|it| it.name.as_str()),
                    AstKind::ArrowFunctionExpression(_) => {
                        get_declaration_identifier(nodes, node.id())
                    }
                    _ => unreachable!(),
                },
            )
        })
        .any(|(ix, id)| {
            id.is_some_and(|name| {
                is_react_component_name(name)
                    || is_react_hook_name(name)
                    || is_memo_or_forward_ref_callback(nodes, ix)
            })
        })
}

fn get_declaration_identifier<'a>(nodes: &'a AstNodes<'a>, node_id: AstNodeId) -> Option<&str> {
    nodes.ancestors(node_id).map(|id| nodes.get_node(id)).find_map(|node| {
        if let AstKind::VariableDeclaration(decl) = node.kind() {
            if decl.declarations.len() == 1 {
                decl.declarations[0].id.get_identifier().map(Atom::as_str)
            } else {
                None
            }
        } else {
            None
        }
    })
}

/// # Panics
/// `node_id` should always point to a valid `Function`.
fn is_memo_or_forward_ref_callback(nodes: &AstNodes, node_id: AstNodeId) -> bool {
    nodes.ancestors(node_id).map(|id| nodes.get_node(id)).any(|node| {
        if let AstKind::CallExpression(call) = node.kind() {
            call.callee_name().is_some_and(|name| matches!(name, "forwardRef" | "memo"))
        } else {
            false
        }
    })
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
        // Valid -- this is a regression test.
        "
            jest.useFakeTimers();
            beforeEach(() => {
              jest.useRealTimers();
            })
        ",
        // Valid because they're not matching use[A-Z].
        "
            fooState();
            _use();
            _useState();
            use_hook();
            // also valid because it's not matching the PascalCase namespace
            jest.useFakeTimer()
        ",
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
            function RegressionTest(test) {
              while (test) {
                test = update(test);
              }
              React.useLayoutEffect(() => {});
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
        // errors: [classError('This.useHook'), classError('Super.useHook')],
        "
            class C {
                 m() {
                     This.useHook();
                     Super.useHook();
                 }
            }
        ",
        // This is a false positive (it's valid) that unfortunately
        // we cannot avoid. Prefer to rename it to not start with "use"
        // errors: [classError('FooStore.useFeatureFlag')],
        "
            class Foo extends Component {
                render() {
                    if (cond) {
                        FooStore.useFeatureFlag();
                    }
                }
            }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [conditionalError('Namespace.useConditionalHook')],
        "
            function ComponentWithConditionalHook() {
                if (cond) {
                    Namespace.useConditionalHook();
                }
            }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [conditionalError('useConditionalHook')],
        "
                function createComponent() {
                    return function ComponentWithConditionalHook() {
                        if (cond) {
                            useConditionalHook();
                        }
                    }
                }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [conditionalError('useConditionalHook')],
        "
                function useHookWithConditionalHook() {
                    if (cond) {
                        useConditionalHook();
                    }
                }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [conditionalError('useConditionalHook')],
        "
                function createHook() {
                    return function useHookWithConditionalHook() {
                        if (cond) {
                            useConditionalHook();
                        }
                    }
                }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [conditionalError('useTernaryHook')],
        "
                function ComponentWithTernaryHook() {
                    cond ? useTernaryHook() : null;
                }
        ",
        // Invalid because it's a common misunderstanding.
        // We *could* make it valid but the runtime error could be confusing.
        // errors: [genericError('useHookInsideCallback')],
        "
                function ComponentWithHookInsideCallback() {
                    useEffect(() => {
                        useHookInsideCallback();
                    });
                }
        ",
        // Invalid because it's a common misunderstanding.
        // We *could* make it valid but the runtime error could be confusing.
        // errors: [genericError('useHookInsideCallback')],
        "
                function createComponent() {
                    return function ComponentWithHookInsideCallback() {
                        useEffect(() => {
                            useHookInsideCallback();
                        });
                    }
                }
        ",
        // Invalid because it's a common misunderstanding.
        // We *could* make it valid but the runtime error could be confusing.
        // errors: [genericError('useHookInsideCallback')],
        "
                const ComponentWithHookInsideCallback = React.forwardRef((props, ref) => {
                    useEffect(() => {
                        useHookInsideCallback();
                    });
                    return <button {...props} ref={ref} />
                });
        ",
        // Invalid because it's a common misunderstanding.
        // We *could* make it valid but the runtime error could be confusing.
        // errors: [genericError('useHookInsideCallback')],
        "
                const ComponentWithHookInsideCallback = React.memo(props => {
                    useEffect(() => {
                        useHookInsideCallback();
                    });
                    return <button {...props} />
                });
        ",
        // Invalid because it's a common misunderstanding.
        // We *could* make it valid but the runtime error could be confusing.
        // errors: [functionError('useState', 'handleClick')],
        "
                function ComponentWithHookInsideCallback() {
                    function handleClick() {
                        useState();
                    }
                }
        ",
        // Invalid because it's a common misunderstanding.
        // We *could* make it valid but the runtime error could be confusing.
        // errors: [functionError('useState', 'handleClick')],
        "
                function createComponent() {
                    return function ComponentWithHookInsideCallback() {
                        function handleClick() {
                            useState();
                        }
                    }
                }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [loopError('useHookInsideLoop')],
        "
                function ComponentWithHookInsideLoop() {
                    while (cond) {
                        useHookInsideLoop();
                    }
                }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [functionError('useState', 'renderItem')],
        "
                function renderItem() {
                    useState();
                }

                function List(props) {
                    return props.items.map(renderItem);
                }
        ",
        // Currently invalid because it violates the convention and removes the "taint"
        // from a hook. We *could* make it valid to avoid some false positives but let's
        // ensure that we don't break the "renderItem" and "normalFunctionWithConditionalHook"
        // cases which must remain invalid.
        // errors: [functionError('useHookInsideNormalFunction', 'normalFunctionWithHook'), ],
        "
                function normalFunctionWithHook() {
                    useHookInsideNormalFunction();
                }
        ",
        // These are neither functions nor hooks.
        // errors: [
        //     functionError('useHookInsideNormalFunction', '_normalFunctionWithHook'),
        //     functionError('useHookInsideNormalFunction', '_useNotAHook'),
        // ],
        "
                function _normalFunctionWithHook() {
                    useHookInsideNormalFunction();
                }
                function _useNotAHook() {
                    useHookInsideNormalFunction();
                }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [
        //   functionError(
        //     'useHookInsideNormalFunction',
        //     'normalFunctionWithConditionalHook'
        //   ),
        // ],
        "
                function normalFunctionWithConditionalHook() {
                    if (cond) {
                        useHookInsideNormalFunction();
                    }
                }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [
        //     loopError('useHook1'),
        //     loopError('useHook2'),
        //     loopError('useHook3'),
        //     loopError('useHook4'),
        // ]
        "
                function useHookInLoops() {
                    while (a) {
                        useHook1();
                        if (b) return;
                        useHook2();
                    }
                    while (c) {
                        useHook3();
                        if (d) return;
                        useHook4();
                    }
                }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [loopError('useHook1'), loopError('useHook2', true)],
        "
            function useHookInLoops() {
                while (a) {
                    useHook1();
                    if (b) continue;
                    useHook2();
                }
            }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [conditionalError('useHook')],
        // TODO: FIX ME!
        // "
        //         function useLabeledBlock() {
        //             label: {
        //                 if (a) break label;
        //                 useHook();
        //             }
        //         }
        // ",
        // Currently invalid.
        // These are variations capturing the current heuristic--
        // we only allow hooks in PascalCase or useFoo functions.
        // We *could* make some of these valid. But before doing it,
        // consider specific cases documented above that contain reasoning.
        // errors: [
        //     functionError('useState', 'a'),
        //     functionError('useState', 'b'),
        //     functionError('useState', 'c'),
        //     functionError('useState', 'd'),
        //     functionError('useState', 'e'),
        //     functionError('useState', 'f'),
        //     functionError('useState', 'g'),
        //     functionError('useState', 'j'),
        //     functionError('useState', 'k'),
        // ]
        "
            function a() { useState(); }
            const whatever = function b() { useState(); };
            const c = () => { useState(); };
            let d = () => useState();
            e = () => { useState(); };
            ({f: () => { useState(); }});
            ({g() { useState(); }});
            const {j = () => { useState(); }} = {};
            ({k = () => { useState(); }} = {});
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [conditionalError('useState', true)],
        "
                function useHook() {
                    if (a) return;
                    useState();
                }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [conditionalError('useState', true)],
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
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [conditionalError('useState', true)],
        "
                function useHook() {
                    if (b) {
                        console.log('true');
                    } else {
                        console.log('false');
                    }
                    if (a) return;
                    useState();
                }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [conditionalError('useHook1'), conditionalError('useHook2')],
        "
                function useHook() {
                    a && useHook1();
                    b && useHook2();
                }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [
        //     // NOTE: This is an error since `f()` could possibly throw.
        //     conditionalError('useState'),
        // ],
        "
                function useHook() {
                    try {
                        f();
                        useState();
                    } catch {}
                }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [
        //     conditionalError('useState'),
        //     conditionalError('useState'),
        //     conditionalError('useState'),
        // ],
        "
                function useHook({ bar }) {
                    let foo1 = bar && useState();
                    let foo2 = bar || useState();
                    let foo3 = bar ?? useState();
                }
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [conditionalError('useCustomHook')],
        "
                const FancyButton = React.forwardRef((props, ref) => {
                    if (props.fancy) {
                        useCustomHook();
                    }
                    return <button ref={ref}>{props.children}</button>;
                });
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [conditionalError('useCustomHook')],
        "
                const FancyButton = forwardRef(function(props, ref) {
                    if (props.fancy) {
                        useCustomHook();
                    }
                    return <button ref={ref}>{props.children}</button>;
                });
        ",
        // Invalid because it's dangerous and might not warn otherwise.
        // This *must* be invalid.
        // errors: [conditionalError('useCustomHook')],
        "
                const MemoizedButton = memo(function(props) {
                    if (props.fancy) {
                        useCustomHook();
                    }
                    return <button>{props.children}</button>;
                });
        ",
        // This is invalid because "use"-prefixed functions used in named
        // functions are assumed to be hooks.
        // errors: [functionError('useProbablyAHook', 'notAComponent')],
        "
                React.unknownFunction(function notAComponent(foo, bar) {
                    useProbablyAHook(bar)
                });
        ",
        // Invalid because it's dangerous.
        // Normally, this would crash, but not if you use inline requires.
        // This *must* be invalid.
        // It's expected to have some false positives, but arguably
        // they are confusing anyway due to the use*() convention
        // already being associated with Hooks.
        // errors: [
        //     topLevelError('useState'),
        //     topLevelError('React.useCallback'),
        //     topLevelError('useCustomHook'),
        // ],
        "
            useState();
            if (foo) {
                const foo = React.useCallback(() => {});
            }
            useCustomHook();
        ",
        // Technically this is a false positive.
        // We *could* make it valid (and it used to be).
        //
        // However, top-level Hook-like calls can be very dangerous
        // in environments with inline requires because they can mask
        // the runtime error by accident.
        // So we prefer to disallow it despite the false positive.
        // errors: [topLevelError('useBasename')],
        "
            const {createHistory, useBasename} = require('history-2.1.2');
            const browserHistory = useBasename(createHistory)({
                basename: '/',
            });
        ",
        // errors: [classError('useFeatureFlag')],
        "
                class ClassComponentWithFeatureFlag extends React.Component {
                    render() {
                        if (foo) {
                            useFeatureFlag();
                        }
                    }
                }
        ",
        // errors: [classError('React.useState')],
        "
                class ClassComponentWithHook extends React.Component {
                    render() {
                        React.useState();
                    }
                }
        ",
        // errors: [classError('useState')],
        "(class {useHook = () => { useState(); }});",
        // errors: [classError('useState')],
        "(class {useHook() { useState(); }});",
        // errors: [classError('useState')],
        "(class {h = () => { useState(); }});",
        // errors: [classError('useState')],
        "(class {i() { useState(); }});",
        // errors: [asyncComponentHookError('useState')],
        "
                async function AsyncComponent() {
                    useState();
                }
        ",
        // errors: [asyncComponentHookError('useState')],
        "
                async function useAsyncHook() {
                    useState();
                }
        ",
        // errors: [
        //     topLevelError('Hook.use'),
        //     topLevelError('Hook.useState'),
        //     topLevelError('Hook.use42'),
        //     topLevelError('Hook.useHook'),
        // ],
        "
            Hook.use();
            Hook._use();
            Hook.useState();
            Hook._useState();
            Hook.use42();
            Hook.useHook();
            Hook.use_hook();
        ",
        // errors: [functionError('use', 'notAComponent')],
        "
                function notAComponent() {
                    use(promise);
                }
        ",
        // errors: [topLevelError('use')],
        "
            const text = use(promise);
            function App() {
                return <Text text={text} />
            }
        ",
        // errors: [classError('use')],
        "
            class C {
                m() {
                    use(promise);
                }
            }
        ",
        // errors: [asyncComponentHookError('use')],
        "
            async function AsyncComponent() {
                    use();
            }
        ",
    ];

    Tester::new(RulesOfHooks::NAME, pass, fail).test_and_snapshot();
}
