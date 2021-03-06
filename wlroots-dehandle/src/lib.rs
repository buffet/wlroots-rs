extern crate proc_macro;
extern crate proc_macro2;
#[macro_use]
extern crate syn;
#[macro_use]
extern crate quote;

use std::collections::HashMap;

use proc_macro::TokenStream;
use proc_macro2::Ident;
use syn::{ItemFn, Stmt, UseTree, ItemUse, Item, Block, Expr,
          parse::{self, Parse, ParseStream},
          punctuated::Punctuated,
          fold::Fold};

/// Parses a list of variable names separated by commas
///
/// This is how the compiler passes in arguments to our attribute -- it is
/// everything inside the delimiters after the attribute name.
///```rust,ignore
///     #[wlroots_dehandle(a, b, c)]
///```
struct Args {
    vars: HashMap<Ident, bool>
}

impl Parse for Args {
    fn parse(input: ParseStream) -> parse::Result<Self> {
        let vars = Punctuated::<Ident, Token![,]>::parse_terminated(input)?;
        Ok(Args {
            vars: vars.into_iter().map(|k| (k, false)).collect(),
        })
    }
}

impl Fold for Args {
    fn fold_block(&mut self, block: Block) -> Block {
        build_block(block.stmts.iter(), self)
    }
}

impl Args {
    fn is_handle(&mut self, name: Ident) -> bool {
        if let Some(seen) = self.vars.get_mut(&name) {
            *seen = true;
            true
        } else {
            false
        }
    }
}

/// Attribute to automatically call the `run` method on handles with the
/// remaining block of code.
///
/// The name of the variable you want to use as the upgraded handle should be
/// provided as an argument to the attribute. It does not need to be the same
/// as the handle variable.
///
/// The syntax in the code should be `use $handle as $upgraded_handle`.
/// E.g the variable in the code that stores the handle should go on the
/// **left** and the variable you used in the attribute declaration should
/// go on the **right**.
///
/// # Panics
/// If the handle is invalid (e.g. default constructed, or is a dangling
/// handle) then your code will `panic!`.
///
/// If this is undesirable, please use the non-proc macro `with_handles!`.
///
/// # Example
///
/// ```rust,ignore
/// impl InputManagerHandler for InputManager {
///     #[wlroots_dehandle(compositor, keyboard, seat)]
///     fn keyboard_added(&mut self,
///                       compositor_handle: CompositorHandle,
///                       keyboard: KeyboardHandle)
///                       -> Option<Box<Keyboard Handler>> {
///         {
///             use compositor_handle as compositor;
///             use keyboard as keyboard;
///             let server: &mut ::Server = compositor.into();
///             server.keyboards.push(keyboard.weak_reference());
///             // Now that we have at least one keyboard, update the seat capabilities.
///             let server_seat = &server.seat.seat;
///             use server_seat as seat;
///             let mut capabilities = seat.capabilities();
///             capabilities.insert(Capability::Keyboard);
///             seat.set_capabilities(capabilities);
///             seat.set_keyboard(keyboard.input_device());
///         }
///         // Due to some weird closure inference rules, this has to be outside
///         // of the above block.
///         Some(Box::new(::Keyboard))
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn wlroots_dehandle(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemFn);
    let mut args = parse_macro_input!(args as Args);
    if args.vars.len() == 0 {
        panic!("wlroots_dehandle requires at least one argument")
    }
    let output = args.fold_item_fn(input);
    for (arg, seen) in args.vars {
        if !seen {
            panic!(format!("Must use all declared handles, didn't use `{}`",
                           arg))
        }
    }
    TokenStream::from(quote!(#output))
}

fn build_block(mut input: std::slice::Iter<Stmt>, args: &mut Args) -> Block {
    let mut output = vec![];
    let mut inner = None;
    while let Some(stmt) = input.next().cloned() {
        use {Item::Use, UseTree::Rename};
        match stmt.clone() {
            Stmt::Item(Use(ItemUse { tree: Rename(use_stmt), ..})) => {
                if args.is_handle(use_stmt.rename.clone()) {
                    inner = Some((use_stmt.ident, use_stmt.rename));
                    break
                }
                output.push(stmt)
            },
            // Recurse into function body
            Stmt::Item(Item::Fn(mut function)) => {
                let inner_block = function.block.clone();
                *function.block = build_block(inner_block.stmts.iter(), args);
                output.push(Stmt::Item(Item::Fn(function)))
            },
            // Recurse into let call
            Stmt::Local(mut local) => {
                if let Some((_, body)) = local.init.clone() {
                    let body = build_block_expr(*body.clone(), args);
                    let body = parse_quote!(#body);
                    local.init.as_mut().unwrap().1 = body;
                    output.push(Stmt::Local(local))
                }
            },
            Stmt::Expr(expr) => {
                let body = build_block_expr(expr, args);
                output.push(parse_quote!({#body}))
            }
            Stmt::Semi(expr, _) => {
                let body = build_block_expr(expr, args);
                output.push(parse_quote!({#body;}))
            }
            _ => output.push(stmt)
        }
    }
    if let Some((handle, dehandle)) = inner {
        let inner_block = build_block(input, args);
        let handle_call = parse_quote!(
            {(#handle).run(|#dehandle|{
                #inner_block
            }).expect(concat!("Could not upgrade handle ",
                              stringify!(#handle), " to ",
                              stringify!(#dehandle)))}
        );
        output.push(handle_call);
    }
    parse_quote!({#(#output)*})
}

/// Tries to build a block from the expression.
fn build_block_expr(expr: Expr, args: &mut Args) -> Expr {
    match expr {
        Expr::Block(block) => {
            let block = build_block(block.block.stmts.iter(), args);
            parse_quote!(#block)
        }
        Expr::Let(mut let_expr) => {
            *let_expr.expr = build_block_expr(*let_expr.expr.clone(), args);
            Expr::Let(let_expr)
        },
        Expr::If(mut if_expr) => {
            let then_branch = if_expr.then_branch.clone();
            let then_branch = build_block_expr(parse_quote!(#then_branch), args);
            if_expr.then_branch = parse_quote!(#then_branch);
            if_expr.else_branch = match if_expr.else_branch.clone() {
                None => if_expr.else_branch,
                Some((token, else_branch)) => {
                    Some((token, Box::new( build_block_expr(*else_branch, args))))
                }
            };
            Expr::If(if_expr)
        },
        Expr::While(mut while_expr) => {
            let body = while_expr.body.clone();
            let body = build_block_expr(parse_quote!(#body), args);
            while_expr.body = parse_quote!(#body);
            Expr::While(while_expr)
        },
        Expr::ForLoop(mut for_expr) => {
            let body = for_expr.body.clone();
            let body = build_block_expr(parse_quote!(#body), args);
            for_expr.body = parse_quote!(#body);
            Expr::ForLoop(for_expr)
        },
        Expr::Loop(mut loop_expr) => {
            let body = loop_expr.body.clone();
            let body = build_block_expr(parse_quote!(#body), args);
            loop_expr.body = parse_quote!(#body);
            Expr::Loop(loop_expr)
        },
        Expr::Match(mut match_expr) => {
            for arm in &mut match_expr.arms {
                *arm.body = build_block_expr(*arm.body.clone(), args)
            }
            Expr::Match(match_expr)
        },
        Expr::Struct(mut struct_expr) => {
            for field in &mut struct_expr.fields {
                field.expr = build_block_expr(field.expr.clone(), args);
            }
            Expr::Struct(struct_expr)
        },
        Expr::Call(mut call_expr) => {
            for arg in &mut call_expr.args {
                *arg = build_block_expr(arg.clone(), args);
            }
            Expr::Call(call_expr)
        },
        Expr::MethodCall(mut call_expr) => {
            for arg in &mut call_expr.args {
                *arg = build_block_expr(arg.clone(), args);
            }
            Expr::MethodCall(call_expr)
        },
        Expr::Closure(mut closure_expr) => {
            *closure_expr.body = build_block_expr(*closure_expr.body.clone(),
                                                  args);
            Expr::Closure(closure_expr)
        },
        Expr::Unsafe(mut unsafe_expr) => {
            unsafe_expr.block = build_block(unsafe_expr.block.stmts.iter(),
                                            args);
            Expr::Unsafe(unsafe_expr)
        },
        Expr::Assign(mut assign_expr) => {
            *assign_expr.right = build_block_expr(*assign_expr.right.clone(),
                                                  args);
            Expr::Assign(assign_expr)
        },
        Expr::AssignOp(mut assign_expr) => {
            *assign_expr.right = build_block_expr(*assign_expr.right.clone(),
                                                  args);
            Expr::AssignOp(assign_expr)
        },
        Expr::Break(mut break_expr) => {
            match break_expr.expr {
                None => {},
                Some(ref mut expr) => {
                    **expr = build_block_expr(*expr.clone(), args);
                }
            }
            Expr::Break(break_expr)
        },
        Expr::Return(mut return_expr) => {
            match return_expr.expr {
                None => {},
                Some(ref mut expr) => {
                    **expr = build_block_expr(*expr.clone(), args);
                }
            }
            Expr::Return(return_expr)
        },
        Expr::Reference(mut reference_expr) => {
            *reference_expr.expr = build_block_expr(*reference_expr.expr.clone(),
                                                    args);
            Expr::Reference(reference_expr)
        },
        Expr::Paren(mut paren_expr) => {
            *paren_expr.expr = build_block_expr(*paren_expr.expr.clone(), args);
            Expr::Paren(paren_expr)
        },
        Expr::Unary(mut unary_expr) => {
            *unary_expr.expr = build_block_expr(*unary_expr.expr.clone(), args);
            Expr::Unary(unary_expr)
        },
        Expr::Binary(mut binary_expr) => {
            *binary_expr.left = build_block_expr(*binary_expr.left.clone(), args);
            *binary_expr.right = build_block_expr(*binary_expr.right.clone(), args);
            Expr::Binary(binary_expr)
        },
        v => {
            v
        }
    }
}
