use std::fmt;
use std::str;

use rquickjs::{
    Atom, Class, Coerced, Context as RContext, Ctx, Error, Exception, Function, IntoJs, Persistent,
    Runtime, Type, class::Trace, context::intrinsic::*, function::Opt,
};
// use rquickjs::{embed, loader::Bundle};

use crate::helpers::number::Interval;
use crate::io::Record;
use crate::var::symbols::{OptValue, SymbolTable, Value};

use super::{ExprContext, Expression, Var};

#[derive(Trace, Default, rquickjs::JsLifetime)]
#[rquickjs::class(rename = "Interval")]
pub struct JsInterval {
    #[qjs(get, set)]
    pub start: f64,

    #[qjs(get, set)]
    pub end: f64,
}

#[rquickjs::methods]
impl JsInterval {
    #[qjs(constructor)]
    pub fn new(start: f64, end: f64) -> Self {
        JsInterval { start, end }
    }
}

#[rquickjs::function]
pub fn bin(x: Coerced<f64>, interval: Opt<Coerced<f64>>) -> rquickjs::Result<JsInterval> {
    let out = crate::helpers::number::bin(x.0, interval.map(|i| i.0).unwrap_or(1.));
    Ok(JsInterval::new(out.0.inner(), out.1.inner()))
}

// // TODO: 'num' and 'int' functions are implemented in JS, not sure how to do it in Rust (call parseFloat and parseInt from here?)
// #[rquickjs::function]
// pub fn num(x: rquickjs::Value<'_>) -> rquickjs::Result<f64> {
//     unimplemented!()
// }

include!("_js_include.rs");
// static INCLUDE: Bundle = embed!{
//     "globals": "../js/include.js",
// };

fn register_globals(ctx: &Ctx) {
    // classes/functions in Rust
    ctx.globals().set("bin", js_bin).unwrap();
    Class::<JsInterval>::create_constructor(ctx).unwrap();
    Class::<JsInterval>::define(&ctx.globals()).unwrap();
    // "standard library written in JS"
    let _: () = ctx.eval(JS_INCLUDE.as_bytes()).unwrap();
}

fn to_js_value<'a>(
    value: Option<&Value>,
    record: &dyn Record,
    ctx: Ctx<'a>,
) -> Result<rquickjs::Value<'a>, String> {
    let res = if let Some(v) = value {
        match v {
            Value::Bool(v) => v.get().into_js(&ctx),
            Value::Int(v) => v.get().into_js(&ctx),
            Value::Float(v) => v.get().into_js(&ctx),
            Value::Interval(v) => {
                let int = v.get();
                let cls = Class::instance(ctx.clone(), JsInterval::new(*int.0, *int.1)).unwrap();
                Ok(cls.into_value())
            }
            Value::Text(v) => v.as_str(record, |s| s.into_js(&ctx))?,
            Value::Attr(v) => v
                .with_str(record, |v| v.into_js(&ctx))
                .map_err(|e| e.to_string())?,
        }
    } else {
        ().into_js(&ctx)
    };
    res.map_err(|e| e.to_string())
}

fn write_value(v: &rquickjs::Value, out: &mut OptValue) -> Result<(), String> {
    #[inline(never)]
    fn write_err(ty: &Type) -> String {
        format!("Expression returned a type that cannot be interpreted: {ty}")
    }
    let ty = v.type_of();
    match ty {
        Type::Bool => out.inner_mut().set_bool(v.as_bool().unwrap()),
        Type::Int | Type::BigInt => out.inner_mut().set_int(v.as_int().unwrap() as i64),
        Type::Float => out.inner_mut().set_float(v.as_float().unwrap()),
        Type::String => out.inner_mut().set_text(
            v.as_string()
                .unwrap()
                .to_string()
                .map_err(|_| format!("Expression error: Could not convert {v:?} to string"))?
                .as_bytes(),
        ),
        Type::Undefined | Type::Null => out.set_none(),
        Type::Object => {
            if let Ok(obj) = Class::<JsInterval>::from_value(v) {
                let int = obj.borrow();
                out.inner_mut()
                    .set_interval(Interval::new(int.start, int.end));
            } else {
                return Err(write_err(&ty));
            }
        }
        _ => return Err(write_err(&ty)),
    }
    Ok(())
}

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
            Eval,
            Json,
            RegExp,
            RegExpCompiler,
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
    fn init(&mut self, init_code: Option<&str>) -> Result<(), String> {
        // println!("init: {:?}", init_code);
        self.context.with(|ctx: Ctx<'_>| {
            // global functions/classes
            register_globals(&ctx);
            // initialization code
            if let Some(code) = init_code {
                let _: () = ctx
                    .eval(code.as_bytes())
                    .map_err(|e| obtain_exception(e, ctx.clone()))?;
            }
            Ok::<_, String>(())
        })?;
        Ok(())
    }

    // fn clear(&mut self) {
    //     self.vars.clear();
    // }

    fn register(&mut self, var: &Var) -> Result<(), String> {
        if !self.vars.iter().any(|(v, _)| *v == var.symbol_id) {
            self.context.with(|ctx| {
                let atom = Persistent::save(&ctx, Atom::from_str(ctx.clone(), &var.name).unwrap());
                self.vars.push((var.symbol_id, atom));
            });
        }
        Ok(())
    }

    fn next_record(
        &mut self,
        symbols: &SymbolTable,
        record: &dyn Record,
    ) -> Result<(), (usize, String)> {
        // copy values from symbol table to context
        // eprintln!("fill {:?}", symbols);
        self.context.with(|ctx| {
            let globals = ctx.globals();
            for (var_id, atom) in &self.vars {
                let val = to_js_value(symbols.get(*var_id).inner(), record, ctx.clone())
                    .map_err(|e| (*var_id, e))?;
                let _atom = atom.clone().restore(&ctx).unwrap();
                globals.set(_atom, val).unwrap();
            }
            Ok(())
        })
    }
}

#[derive(Debug, Default)]
pub struct JsExpr {
    func: Option<Persistent<Function<'static>>>,
}

impl Expression for JsExpr {
    type Context = Context;

    fn register(
        &mut self,
        expr_id: usize,
        expr: &str,
        engine: &mut Self::Context,
    ) -> Result<(), String> {
        // println!("register js {}", expr);
        let fn_name = format!("____eval_{expr_id}");
        let func = engine.context.with(|ctx| {
            let arrow_script = format!("{fn_name} => ({expr})");
            // println!("arrow: {:?}", arrow_script);
            let func: Function = match ctx.eval(arrow_script) {
                Ok(rv) => rv,
                Err(_) => {
                    // not a valid arrow function, try regular function (assumes a return statement to be present)
                    let fn_script = format!("var {fn_name} = function() {{ {expr} }}");
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

    fn eval(&mut self, out: &mut OptValue, engine: &mut Self::Context) -> Result<(), String> {
        // println!("eval js");
        engine.context.with(|ctx| {
            let _func = self.func.clone().unwrap().restore(&ctx.clone()).unwrap();
            let res: rquickjs::Value<'_> = (_func)
                .call(())
                .map_err(|e| obtain_exception(e, ctx.clone()).to_string())?;
            // println!("res {:?}", res);
            write_value(&res, out)?;
            Ok::<_, String>(())
        })
    }
}

fn obtain_exception(e: Error, ctx: Ctx<'_>) -> String {
    let msg = if let Error::Exception = e {
        let v = ctx.catch();
        match v.type_of() {
            Type::Exception => Exception::from_object(v.into_object().unwrap())
                .and_then(|o| o.message())
                .unwrap_or_else(|| "Unknown error".to_string()),
            Type::String => v
                .as_string()
                .unwrap()
                .to_string()
                .unwrap_or_else(|_| format!("{v:?}")),
            _ => format!("{v:?}"),
        }
    } else {
        e.to_string()
    };
    format!("JavaScript error: {msg}")
}
