extern crate proc_macro;

use itertools::Itertools;
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Attribute, Expr, Ident, Lifetime, Lit, Meta, Token, Type,
};

#[derive(Debug)]
struct VarMod {
    name: Ident,
    lifetime: Option<Lifetime>,
    description: String,
    vars: Punctuated<Func, Token![,]>,
}

impl Parse for VarMod {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attr = Attribute::parse_outer(input)?;
        let description = extract_docstring(&attr).unwrap_or("".to_string());
        let name = input.parse()?;
        let lifetime = if input.peek(Token![<]) {
            let _: Token![<] = input.parse()?;
            let lt = input.parse()?;
            let _: Token![>] = input.parse()?;
            Some(lt)
        } else {
            None
        };
        let content;
        braced!(content in input);
        let vars = content.parse_terminated(Func::parse, Token![,])?;
        Ok(Self {
            name,
            lifetime,
            description,
            vars,
        })
    }
}

#[derive(Debug)]
struct Func {
    name: Ident,
    description: String,
    args: Option<Punctuated<Arg, Token![,]>>,
    output_type: Option<Ident>,
    hide_from_usage: bool,
}

impl Parse for Func {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = Attribute::parse_outer(input)?;
        let description = extract_docstring(&attrs).unwrap_or("".to_string());
        let hide_from_usage = attrs.iter().any(|a| a.path().is_ident("hidden"));
        let name = input.parse()?;
        let content;
        parenthesized!(content in input);
        let output_type = if content.peek(Token![?]) {
            let _: Token![?] = content.parse()?;
            None
        } else {
            Some(content.parse()?)
        };
        let args = if input.peek(Token![,]) {
            None
        } else {
            let content;
            braced!(content in input);
            Some(content.parse_terminated(Arg::parse, Token![,])?)
        };
        // let _: Token![,] = input.parse()?;
        Ok(Self {
            name,
            description,
            args,
            output_type,
            hide_from_usage,
        })
    }
}

#[derive(Debug)]
struct Arg {
    name: Ident,
    ty: Type,
    default_value: Option<Expr>,
}

impl Parse for Arg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty = input.parse()?;
        let equal_token: Option<Token![=]> = input.parse()?;
        let default_value = if equal_token.is_some() {
            Some(input.parse()?)
        } else {
            None
        };
        Ok(Self {
            name,
            ty,
            default_value,
        })
    }
}

#[proc_macro]
pub fn variable_enum(input: TokenStream) -> TokenStream {
    let varmod = parse_macro_input!(input as VarMod);
    let desc = &varmod.description;
    let (title, desc, examples) = parse_mod_desc(desc);
    let examples = examples.into_iter().map(|(d, e, o)| {
        let o = match o {
            Some(o) => quote!( Some(#o) ),
            None => quote!(None),
        };
        quote! {
            var_provider::UsageExample {
                description: #d,
                command: #e,
                output: #o,
            }
        }
    });
    let enum_name = &varmod.name;
    let enum_def = generate_enum(&varmod);
    let usage_impl = generate_usage(&varmod);
    let from_func_impl = generate_from(&varmod);
    let lifetime = varmod.lifetime.as_ref().map(|lt| quote!(<#lt>));
    let expanded = quote! {
        #enum_def

        impl #lifetime #enum_name #lifetime {
            #from_func_impl
        }

        impl #lifetime var_provider::VarProviderInfo for #enum_name #lifetime {
            const TITLE: &'static str = #title;
            const DESC: &'static str = #desc;
            const VARS: &'static [var_provider::FuncUsage] = #usage_impl;
            const EXAMPLES: &'static [var_provider::UsageExample] = &[ #( #examples ),* ];
        }
    };

    TokenStream::from(expanded)
}

fn generate_enum(varmod: &VarMod) -> proc_macro2::TokenStream {
    let enum_variants = varmod.vars.iter().map(|var| {
        let name = &var.name;
        if let Some(args) = var.args.as_ref() {
            let _args = args.iter().map(|arg| {
                let name = &arg.name;
                let ty = &arg.ty;
                quote! { #name: #ty }
            });
            quote! {
                #name { #(#_args,)* }
            }
        } else {
            quote! { #name }
        }
    });
    let name = &varmod.name;
    let lifetime = varmod.lifetime.as_ref().map(|lt| quote!(<#lt>));
    quote! {
        #[derive(Debug, Clone, PartialEq)]
        pub enum #name #lifetime {
            #(#enum_variants),*
        }
    }
}

fn generate_usage(varmod: &VarMod) -> proc_macro2::TokenStream {
    let usage_list = varmod.vars.iter().map(|var| {
        let args = var.args.iter().flat_map(|_args| {
            _args.iter().map(|arg| {
                let name = arg.name.to_string();
                let def = match arg.default_value.as_ref() {
                    Some(e) => quote! { Some(stringify!(#e)) },
                    None => quote! { None },
                };
                quote! {
                    var_provider::ArgUsage {
                        name: #name,
                        default_value: #def,
                    }
                }
            })
        });
        let name = snake_case(&var.name.to_string());
        let desc = join_desc_lines(var.description.lines());
        let hidden = &var.hide_from_usage;
        let ty = match var.output_type.as_ref() {
            Some(ty) => quote! { Some(var_provider::VarType::#ty) },
            None => quote! { None },
        };
        quote! {
            var_provider::FuncUsage {
                name: #name,
                args: &[#(#args),*],
                description: #desc,
                output_type: #ty,
                hidden: #hidden,
            }
        }
    });
    quote! {
        &[
            #(#usage_list),*
        ]
    }
}

fn generate_from(varmod: &VarMod) -> proc_macro2::TokenStream {
    let varmod_name = &varmod.name;
    let match_arms = varmod.vars.iter().map(|var| {
        let enum_variant = &var.name;
        let var_name_str = snake_case(&enum_variant.to_string());
        let from_impl = var.args.iter().flat_map(|_args| {
            _args.iter().map(|arg| {
                let arg_name = &arg.name;
                let arg_name_str = arg.name.to_string();
                let default_value = match arg.default_value.as_ref() {
                    Some(d) => quote! { Some(#d) },
                    None => quote! { None },
                };
                quote! {
                    let #arg_name = if let Some((_, v)) = args_iter.next() {
                        var_provider::FromArg::from_arg(#var_name_str, #arg_name_str, v)?
                    } else if let Some(v) = #default_value {
                        v
                    } else {
                        return Err(var_provider::missing_argument(#var_name_str, #arg_name_str));
                    };
                }
            })
        });
        let field_list = var
            .args
            .iter()
            .flat_map(|_args| _args.iter().map(|arg| &arg.name));
        let output_type = match var.output_type.as_ref() {
            Some(ty) => quote! { Some(var_provider::VarType::#ty) },
            None => quote! { None },
        };
        quote! {
            #var_name_str => {
                let mut args_iter = args.into_iter().enumerate();
                #(#from_impl)*
                if let Some((i, arg)) = args_iter.next() {
                    return Err(var_provider::too_many_args(#var_name_str, i, arg));
                }
                Ok(Some((
                    #varmod_name::#enum_variant {
                        #(#field_list,)*
                    },
                    #output_type,
                )))
            }
        }
    });
    let mut arg_types = Vec::new();
    let arg_types = &mut arg_types;
    for var in &varmod.vars {
        if let Some(args) = var.args.as_ref() {
            for arg in args {
                if !arg_types.iter().any(|t| t == &arg.ty) {
                    arg_types.push(arg.ty.clone());
                }
            }
        }
    }
    quote! {
        fn from_func<'b, I, A>(name: &str, args: I) -> Result<Option<(Self, Option<var_provider::VarType>)>, String>
        where
            I: IntoIterator<Item = A>,
            A: std::fmt::Display,
            #(#arg_types: var_provider::FromArg<A>),*
        {
            match name {
                #(#match_arms),*
                _ => Ok(None),
            }
        }
    }
}

fn extract_docstring(attrs: &[Attribute]) -> Option<String> {
    let mut out = String::new();
    for attr in attrs {
        if let Meta::NameValue(meta) = &attr.meta {
            if let Expr::Lit(l) = &meta.value {
                if let Lit::Str(doc) = &l.lit {
                    let line = doc.value();
                    out.push_str(if !line.starts_with(' ') {
                        &line
                    } else {
                        &line[1..]
                    });
                    out.push('\n');
                }
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn parse_mod_desc(description: &str) -> (String, String, Vec<(String, String, Option<String>)>) {
    // title/description
    let mut lines = description.lines();
    let title = lines.next().expect("Empty description");
    let title = title.trim();
    assert!(
        title.starts_with("# "),
        "Module description must start with '# <Title>'"
    );
    let parts = lines.group_by(|l| l.trim_start().starts_with("# Examples"));
    let mut parts = parts.into_iter().map(|(_, g)| g);
    let description = join_desc_lines(parts.next().unwrap());
    // examples
    let rest = parts
        .nth(1)
        .expect("No '# Examples' section found")
        .skip(1)
        .map(|l| if l.trim().is_empty() { "" } else { l })
        .join("\n");
    let mut examples = Vec::new();
    for chunk in rest.split("\n\n\n") {
        let mut parts = chunk.split("\n\n");
        let desc = join_desc_lines(parts.next().expect("No example description").lines());
        let cmd = parts
            .next()
            .expect("No example command, should be separated from description by one blank line");
        let cmd = cmd.trim();
        assert!(
            cmd.starts_with('`') && cmd.ends_with('`'),
            "Example command should be enclosed in `quotes`"
        );
        let cmd = cmd[1..cmd.len() - 1].to_string();
        let out = parts.next().map(|s| s.to_string());
        examples.push((desc, cmd, out));
    }
    (title[1..].trim().to_string(), description, examples)
}

fn join_desc_lines<'a, L: IntoIterator<Item = &'a str>>(lines: L) -> String {
    lines
        .into_iter()
        .group_by(|l| l.is_empty())
        .into_iter()
        .map(|(_, chunk)| chunk.into_iter().map(|l| l.trim()).join(" "))
        .filter(|chunk| !chunk.is_empty())
        .join("\n")
}

fn snake_case(name: &str) -> String {
    // taken from Serde code
    let mut snake = String::new();
    for (i, ch) in name.char_indices() {
        if i > 0 && ch.is_uppercase() {
            snake.push('_');
        }
        snake.push(ch.to_ascii_lowercase());
    }
    snake
}
