use std::fmt;
use std::str;

use crate::io::Record;
use crate::var::symbols::OptValue;
use crate::var::symbols::SymbolTable;
use crate::{
    error::{CliError, CliResult},
    var::symbols::Value,
};

use rquickjs::Atom;
use rquickjs::Error;
// use rquickjs::{embed, loader::Bundle};
use rquickjs::{
    context::intrinsic::*, Context as RContext, Ctx, Exception, Function, IntoJs, Persistent,
    Runtime, Type,
};

use super::Var;
use super::{ExprContext, Expression};

// static RESERVED_WORDS: Set<&'static str> = phf_set! {
//     "break", "case", "catch", "class", "const", "continue", "debugger",
//     "default", "delete", "do", "else", "export", "extends", "false",
//     "finally", "for", "function", "if", "import", "in", "instanceof",
//     "new", "null", "return", "super", "switch", "this", "throw", "true",
//     "try", "typeof", "var", "void", "while", "with", "let", "static", "yield",
//     "arguments", "as", "async", "eval", "get", "of", "set", "undefined"
// };

fn to_js_value<'a>(
    value: &OptValue,
    record: &dyn Record,
    ctx: Ctx<'a>,
) -> CliResult<rquickjs::Value<'a>> {
    let out = if let Some(v) = value.value() {
        match v {
            Value::Bool(v) => v.get().into_js(&ctx)?,
            Value::Int(v) => v.get().into_js(&ctx)?,
            Value::Float(v) => v.get().into_js(&ctx)?,
            Value::Text(v) => v.as_str(record, |s| s.into_js(&ctx))??,
            Value::Attr(v) => v.with_str(record, |v| v.into_js(&ctx))??,
        }
    } else {
        ().into_js(&ctx)?
    };
    Ok(out)
}

fn write_value(v: &rquickjs::Value, out: &mut OptValue) -> CliResult<bool> {
    let ty = v.type_of();
    let mut is_bool = false;
    match ty {
        Type::Bool => {
            out.set_bool(v.as_bool().unwrap());
            is_bool = true;
        }
        Type::Int | Type::BigInt => out.set_int(v.as_int().unwrap() as i64),
        Type::Float => out.set_float(v.as_float().unwrap()),
        Type::String => out.set_text(
            v.as_string()
                .unwrap()
                .to_string()
                .map_err(|_| format!("Expression error: Could not convert {:?} to string", v))?
                .as_bytes(),
        ),
        Type::Undefined | Type::Null => out.set_none(),
        _ => {
            return fail!(
                "Expression returned a type that cannot be interpreted: {}",
                ty
            );
        }
    }
    Ok(is_bool)
}

include!("_js_include.rs");
// static INCLUDE: Bundle = embed!{
//     "globals": "../js/include.js",
// };

#[derive(Clone)]
pub struct Context {
    vars: Vec<(usize, Persistent<Atom<'static>>)>,
    context: RContext,
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Context {{ ... }}")
    }
}

impl Default for Context {
    fn default() -> Self {
        let rt = Runtime::new().unwrap();
        let context = RContext::custom::<(
            BaseObjects,
            Eval,
            Json,
            RegExp,
            RegExpCompiler,
            StringNormalize,
            MapSet,
            Date,
            TypedArrays,
        )>(&rt)
        .unwrap();
        // rt.set_loader(INCLUDE, INCLUDE);
        Self {
            context,
            vars: vec![],
        }
    }
}

impl ExprContext for Context {
    fn init(&mut self, init_code: Option<&str>) -> CliResult<()> {
        // println!("init: {:?}", init_code);
        self.context.with(|ctx: Ctx<'_>| {
            ctx.eval(JS_INCLUDE.as_bytes())
                .map_err(|e| obtain_exception(e, ctx.clone()))?;
            if let Some(code) = init_code {
                ctx.eval(code.as_bytes())
                    .map_err(|e| obtain_exception(e, ctx.clone()))?;
            }
            Ok::<_, CliError>(())
        })?;
        Ok(())
    }

    fn clear(&mut self) {
        self.vars.clear();
    }

    fn register(&mut self, var: &Var) -> CliResult<()> {
        if !self.vars.iter().any(|(v, _)| *v == var.symbol_id) {
            self.context.with(|ctx| {
                let atom = Persistent::save(&ctx, Atom::from_str(ctx.clone(), &var.name).unwrap());
                self.vars.push((var.symbol_id, atom));
            });
        }
        Ok(())
    }

    fn fill(
        &mut self,
        symbols: &SymbolTable,
        record: &dyn Record,
    ) -> Result<(), (usize, CliError)> {
        // copy values from symbol table to context
        // eprintln!("fill {:?}", symbols);
        self.context.with(|ctx| {
            let globals = ctx.globals();
            for (var_id, atom) in &self.vars {
                let val = to_js_value(symbols.get(*var_id), record, ctx.clone())
                    .map_err(|e| (*var_id, e))?;
                let _atom = atom.clone().restore(&ctx).unwrap();
                globals.set(_atom, val).unwrap();
            }
            Ok(())
        })
    }
}

#[derive(Debug, Default)]
pub struct Expr {
    func: Option<Persistent<Function<'static>>>,
}

impl Expression for Expr {
    type Context = Context;

    fn register(
        &mut self,
        expr_id: usize,
        expr: &str,
        engine: &mut Self::Context,
    ) -> CliResult<()> {
        // println!("register js {}", expr);
        let fn_name = format!("____eval_{}", expr_id);
        let func = engine.context.with(|ctx| {
            let arrow_script = format!("{} => ({})", fn_name, expr);
            // println!("arrow: {:?}", arrow_script);
            let func: Function = match ctx.eval(arrow_script) {
                Ok(rv) => rv,
                Err(_) => {
                    // not a valid arrow function, try regular function (assumes return statement)
                    let fn_script = format!("var {} = function() {{ {} }}", fn_name, expr);
                    // println!("fn: {:?}", fn_script);
                    ctx.eval::<(), _>(fn_script)
                        .map_err(|e| obtain_exception(e, ctx.clone()))?;
                    ctx.globals().get(fn_name).unwrap()
                }
            };
            Ok::<_, String>(Persistent::save(&ctx, func))
        })?;
        self.func = Some(func);
        Ok(())
    }

    fn eval(&mut self, out: &mut OptValue, engine: &mut Self::Context) -> CliResult<()> {
        // println!("eval js");
        engine.context.with(|ctx| {
            let _func = self.func.clone().unwrap().restore(&ctx.clone()).unwrap();
            let res: rquickjs::Value<'_> = (_func)
                .call(())
                .map_err(|e| obtain_exception(e, ctx.clone()).to_string())?;
            // println!("res {:?}", res);
            write_value(&res, out)?;
            Ok::<_, CliError>(())
        })
    }
}

impl From<rquickjs::Error> for CliError {
    fn from(err: rquickjs::Error) -> CliError {
        CliError::Other(format!("{}", err))
    }
}

fn obtain_exception(e: Error, ctx: Ctx<'_>) -> String {
    if let Error::Exception = e {
        let v = ctx.catch();
        let m = match v.type_of() {
            Type::Exception => Exception::from_object(v.into_object().unwrap())
                .and_then(|o| o.message())
                .unwrap_or_else(|| "Unknown error".to_string()),
            Type::String => v
                .as_string()
                .unwrap()
                .to_string()
                .unwrap_or_else(|_| format!("{:?}", v)),
            _ => format!("{:?}", v),
        };
        format!("Javascript error: {}", m)
    } else {
        format!("Javascript error: {}", e)
    }
}